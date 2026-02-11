#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Running,
    Committing,
    Committed,
    Checkpointed,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub tid: u32,
    pub state: TransactionState,
    pub buffers: BTreeMap<u64, Vec<u8>>,
    pub dirty_list: Vec<u64>,
}

impl Transaction {
    pub fn new(tid: u32) -> Self {
        Self {
            tid,
            state: TransactionState::Running,
            buffers: BTreeMap::new(),
            dirty_list: Vec::new(),
        }
    }

    pub fn add_buffer(&mut self, block_no: u64, original_data: &[u8]) {
        self.buffers
            .entry(block_no)
            .or_insert_with(|| original_data.to_vec());
    }

    pub fn mark_dirty(&mut self, block_no: u64) {
        if !self.dirty_list.contains(&block_no) {
            self.dirty_list.push(block_no);
        }
    }

    pub fn get_dirty_blocks(&self) -> &[u64] {
        &self.dirty_list
    }
}
