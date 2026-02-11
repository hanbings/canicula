#![allow(dead_code)]

use crate::journal::jbd2_superblock::JournalSuperBlock;
use crate::journal::transaction::{Transaction, TransactionState};

pub struct CheckpointManager;

impl CheckpointManager {
    /// Mark committed transactions as checkpointed and reclaim journal tail.
    ///
    /// TODO(journal-checkpoint): The real ext4 checkpoint should verify that every
    /// dirty block in each committed transaction has been written back to its
    /// original filesystem location (via the buffer cache flush path) before
    /// marking the transaction as checkpointed. The current implementation
    /// optimistically marks all Committed transactions as Checkpointed and
    /// resets `s_start = 0`, which is correct only when all dirty data has
    /// already been flushed (e.g. after `unmount` or an explicit `sync`).
    /// Once asynchronous / background writeback is supported, this must be
    /// updated to check per-block writeback status.
    pub fn checkpoint(
        transactions: &mut [Transaction],
        journal_sb: &mut JournalSuperBlock,
    ) -> usize {
        let mut count = 0usize;
        for tx in transactions {
            if tx.state == TransactionState::Committed {
                tx.state = TransactionState::Checkpointed;
                count += 1;
            }
        }
        if count > 0 {
            journal_sb.s_start = 0;
        }
        count
    }
}
