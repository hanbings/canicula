#![allow(dead_code)]

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::journal::commit::JournalCommitter;
use crate::journal::jbd2_superblock::JournalSuperBlock;
use crate::journal::recovery::{JournalRecovery as Jbd2Recovery, RecoverySummary};
use crate::journal::transaction::{Transaction, TransactionState};
use crate::traits::block_device::BlockDevice;
use crate::traits::journal::{Journal, JournalRecovery};

pub struct Jbd2Journal<D: BlockDevice> {
    device: D,
    journal_start_block: u64,
    journal_sb: JournalSuperBlock,
    has_64bit: bool,
    has_csum: bool,
    next_tid: u32,
    running: Option<Transaction>,
    committed: Vec<Transaction>,
}

impl<D: BlockDevice> Jbd2Journal<D> {
    pub fn new(
        device: D,
        journal_start_block: u64,
        journal_sb: JournalSuperBlock,
        has_64bit: bool,
        has_csum: bool,
    ) -> Self {
        Self {
            next_tid: journal_sb.s_sequence,
            device,
            journal_start_block,
            journal_sb,
            has_64bit,
            has_csum,
            running: None,
            committed: Vec::new(),
        }
    }

    pub fn device(&self) -> &D {
        &self.device
    }

    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    pub fn journal_superblock(&self) -> &JournalSuperBlock {
        &self.journal_sb
    }

    pub fn journal_superblock_mut(&mut self) -> &mut JournalSuperBlock {
        &mut self.journal_sb
    }

    pub fn recover_now(&mut self) -> Result<RecoverySummary> {
        Jbd2Recovery::recover(
            &mut self.device,
            self.journal_start_block,
            &mut self.journal_sb,
            self.has_64bit,
            self.has_csum,
        )
    }
}

impl<D: BlockDevice> Journal for Jbd2Journal<D> {
    type Handle = u32;

    fn start_transaction(&mut self) -> Result<Self::Handle> {
        if let Some(tx) = &self.running {
            return Ok(tx.tid);
        }
        let tid = self.next_tid;
        self.next_tid = self.next_tid.wrapping_add(1);
        self.running = Some(Transaction::new(tid));
        Ok(tid)
    }

    fn get_write_access(&mut self, handle: &Self::Handle, block_no: u64) -> Result<()> {
        let tx = self
            .running
            .as_mut()
            .ok_or(Ext4Error::CorruptedFs("no running transaction"))?;
        if tx.tid != *handle {
            return Err(Ext4Error::CorruptedFs("journal handle mismatch"));
        }
        let mut buf = vec![0u8; self.device.block_size()];
        self.device.read_block(block_no, &mut buf)?;
        tx.add_buffer(block_no, &buf);
        Ok(())
    }

    fn dirty_metadata(&mut self, handle: &Self::Handle, block_no: u64) -> Result<()> {
        let tx = self
            .running
            .as_mut()
            .ok_or(Ext4Error::CorruptedFs("no running transaction"))?;
        if tx.tid != *handle {
            return Err(Ext4Error::CorruptedFs("journal handle mismatch"));
        }
        tx.mark_dirty(block_no);
        Ok(())
    }

    fn commit(&mut self, handle: Self::Handle) -> Result<()> {
        let mut tx = self
            .running
            .take()
            .ok_or(Ext4Error::CorruptedFs("no running transaction"))?;
        if tx.tid != handle {
            self.running = Some(tx);
            return Err(Ext4Error::CorruptedFs("journal handle mismatch"));
        }
        JournalCommitter::commit(
            &mut self.device,
            self.journal_start_block,
            &mut self.journal_sb,
            &mut tx,
            self.has_64bit,
            self.has_csum,
        )?;
        if tx.state == TransactionState::Committed {
            self.committed.push(tx);
        }
        Ok(())
    }

    fn abort(&mut self, handle: Self::Handle) -> Result<()> {
        let tx = self
            .running
            .take()
            .ok_or(Ext4Error::CorruptedFs("no running transaction"))?;
        if tx.tid != handle {
            self.running = Some(tx);
            return Err(Ext4Error::CorruptedFs("journal handle mismatch"));
        }
        Ok(())
    }
}

impl<D: BlockDevice> JournalRecovery for Jbd2Journal<D> {
    fn needs_recovery(&self) -> bool {
        Jbd2Recovery::needs_recovery(&self.journal_sb)
    }

    fn recover(&mut self) -> Result<()> {
        let _ = self.recover_now()?;
        Ok(())
    }
}
