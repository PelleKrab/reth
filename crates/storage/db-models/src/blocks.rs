use alloy_eips::eip4895::{Withdrawal, Withdrawals};
use alloy_primitives::{Address, TxNumber};
use core::ops::Range;

/// Total number of transactions.
pub type NumTransactions = u64;

/// The storage of the block body indices.
///
/// It has the pointer to the transaction Number of the first
/// transaction in the block and the total number of transactions.
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy)]
#[cfg_attr(any(test, feature = "arbitrary"), derive(arbitrary::Arbitrary))]
#[cfg_attr(any(test, feature = "reth-codec"), derive(reth_codecs::Compact))]
#[cfg_attr(any(test, feature = "reth-codec"), reth_codecs::add_arbitrary_tests(compact))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredBlockBodyIndices {
    /// The number of the first transaction in this block
    ///
    /// Note: If the block is empty, this is the number of the first transaction
    /// in the next non-empty block.
    pub first_tx_num: TxNumber,
    /// The total number of transactions in the block
    ///
    /// NOTE: Number of transitions is equal to number of transactions with
    /// additional transition for block change if block has block reward or withdrawal.
    pub tx_count: NumTransactions,
}

impl StoredBlockBodyIndices {
    /// Return the range of transaction ids for this block.
    pub const fn tx_num_range(&self) -> Range<TxNumber> {
        self.first_tx_num..self.first_tx_num + self.tx_count
    }

    /// Return the index of last transaction in this block unless the block
    /// is empty in which case it refers to the last transaction in a previous
    /// non-empty block
    pub const fn last_tx_num(&self) -> TxNumber {
        self.first_tx_num.saturating_add(self.tx_count).saturating_sub(1)
    }

    /// First transaction index.
    ///
    /// Caution: If the block is empty, this is the number of the first transaction
    /// in the next non-empty block.
    pub const fn first_tx_num(&self) -> TxNumber {
        self.first_tx_num
    }

    /// Return the index of the next transaction after this block.
    pub const fn next_tx_num(&self) -> TxNumber {
        self.first_tx_num + self.tx_count
    }

    /// Return a flag whether the block is empty
    pub const fn is_empty(&self) -> bool {
        self.tx_count == 0
    }

    /// Return number of transaction inside block
    ///
    /// NOTE: This is not the same as the number of transitions.
    pub const fn tx_count(&self) -> NumTransactions {
        self.tx_count
    }

    /// Returns true if the block contains a transaction with the given number.
    pub const fn contains_tx(&self, tx_num: TxNumber) -> bool {
        tx_num >= self.first_tx_num && tx_num < self.next_tx_num()
    }
}

#[cfg(any(test, feature = "reth-codec"))]
reth_codecs::impl_compression_for_compact!(StoredBlockBodyIndices);

/// The storage representation of block withdrawals.
#[derive(Debug, Default, Eq, PartialEq, Clone)]
#[cfg_attr(any(test, feature = "arbitrary"), derive(arbitrary::Arbitrary))]
#[cfg_attr(any(test, feature = "reth-codec"), reth_codecs::add_arbitrary_tests(compact))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredBlockWithdrawals {
    /// The block withdrawals.
    pub withdrawals: Withdrawals,
}

#[cfg(any(test, feature = "reth-codec"))]
reth_codecs::impl_compression_for_compact!(StoredBlockWithdrawals);

/// A storage representation of block withdrawals that is static file friendly. An inner `None`
/// represents a pre-merge block.
#[derive(Debug, Default, Eq, PartialEq, Clone)]
#[cfg_attr(any(test, feature = "arbitrary"), derive(arbitrary::Arbitrary))]
#[cfg_attr(any(test, feature = "reth-codec"), reth_codecs::add_arbitrary_tests(compact))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StaticFileBlockWithdrawals {
    /// The block withdrawals. A `None` value represents a pre-merge block.
    pub withdrawals: Option<Withdrawals>,
}

#[cfg(any(test, feature = "reth-codec"))]
impl reth_codecs::Compact for StoredBlockWithdrawals {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        encode_withdrawals(&self.withdrawals, buf)
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (withdrawals, buf) = decode_withdrawals(buf, len);
        (Self { withdrawals }, buf)
    }
}

#[cfg(any(test, feature = "reth-codec"))]
impl reth_codecs::Compact for StaticFileBlockWithdrawals {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        let mut len = 0;
        buf.put_u8(self.withdrawals.is_some() as u8);
        len += 1;
        if let Some(withdrawals) = &self.withdrawals {
            len += encode_withdrawals(withdrawals, buf);
        }
        len
    }

    fn from_compact(mut buf: &[u8], len: usize) -> (Self, &[u8]) {
        use bytes::Buf;

        let has_withdrawals = buf.get_u8() == 1;
        if has_withdrawals {
            let (withdrawals, buf) = decode_withdrawals(buf, len.saturating_sub(1));
            (Self { withdrawals: Some(withdrawals) }, buf)
        } else {
            (Self { withdrawals: None }, buf)
        }
    }
}

#[cfg(any(test, feature = "reth-codec"))]
reth_codecs::impl_compression_for_compact!(StaticFileBlockWithdrawals);

#[cfg(any(test, feature = "reth-codec"))]
fn encode_withdrawals<B>(withdrawals: &Withdrawals, buf: &mut B) -> usize
where
    B: bytes::BufMut + AsMut<[u8]>,
{
    use reth_codecs::Compact;

    let mut len = 0;
    len += (withdrawals.len() as u64).to_compact(buf);
    for withdrawal in withdrawals.iter() {
        len += withdrawal.index.to_compact(buf);
        len += withdrawal.validator_index.to_compact(buf);
        buf.put_slice(withdrawal.address.as_slice());
        len += withdrawal.address.as_slice().len();
        len += withdrawal.amount.to_compact(buf);
    }
    len
}

#[cfg(any(test, feature = "reth-codec"))]
fn decode_withdrawals(mut buf: &[u8], len: usize) -> (Withdrawals, &[u8]) {
    use bytes::Buf;
    use reth_codecs::Compact;

    let (count, rest) = u64::from_compact(buf, len);
    buf = rest;
    let mut withdrawals = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (index, next) = u64::from_compact(buf, buf.len());
        buf = next;
        let (validator_index, next) = u64::from_compact(buf, buf.len());
        buf = next;
        let address = Address::from_slice(&buf[..20]);
        buf.advance(20);
        let (amount, next) = u64::from_compact(buf, buf.len());
        buf = next;
        withdrawals.push(Withdrawal { index, validator_index, address, amount });
    }
    (Withdrawals::new(withdrawals), buf)
}

#[cfg(test)]
mod tests {
    use crate::StoredBlockBodyIndices;

    #[test]
    fn block_indices() {
        let first_tx_num = 10;
        let tx_count = 6;
        let block_indices = StoredBlockBodyIndices { first_tx_num, tx_count };

        assert_eq!(block_indices.first_tx_num(), first_tx_num);
        assert_eq!(block_indices.last_tx_num(), first_tx_num + tx_count - 1);
        assert_eq!(block_indices.next_tx_num(), first_tx_num + tx_count);
        assert_eq!(block_indices.tx_count(), tx_count);
        assert_eq!(block_indices.tx_num_range(), first_tx_num..first_tx_num + tx_count);
    }
}
