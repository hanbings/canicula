#![allow(dead_code)]

use crate::journal::jbd2_superblock::JournalSuperBlock;
use crate::journal::transaction::{Transaction, TransactionState};

pub struct CheckpointManager;

impl CheckpointManager {
    /// Mark committed transactions as checkpointed and reclaim journal tail.
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
