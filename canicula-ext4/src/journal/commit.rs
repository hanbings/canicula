#![allow(dead_code)]

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::journal::descriptor::{TAG_FLAG_ESCAPE, TAG_FLAG_LAST_TAG, TAG_FLAG_SAME_UUID};
use crate::journal::jbd2_superblock::{
    JBD2_BLOCKTYPE_COMMIT, JBD2_BLOCKTYPE_DESCRIPTOR, JBD2_MAGIC_NUMBER, JournalSuperBlock,
};
use crate::journal::transaction::{Transaction, TransactionState};
use crate::layout::checksum::crc32c;
use crate::traits::block_device::BlockDevice;

pub struct JournalCommitter;

impl JournalCommitter {
    pub fn commit<D: BlockDevice>(
        device: &mut D,
        journal_start_block: u64,
        journal_sb: &mut JournalSuperBlock,
        txn: &mut Transaction,
        has_64bit: bool,
        has_csum: bool,
    ) -> Result<()> {
        if txn.state != TransactionState::Running {
            return Err(Ext4Error::CorruptedFs("transaction not running"));
        }
        txn.state = TransactionState::Committing;

        let bs = journal_sb.s_blocksize as usize;
        if bs != device.block_size() {
            return Err(Ext4Error::CorruptedFs("journal block size mismatch"));
        }

        let mut pos = if journal_sb.s_start == 0 {
            journal_sb.s_first
        } else {
            journal_sb.s_start
        };

        let mut journal_data = vec![];
        let mut per_tag_flags = vec![];
        let mut tmp = vec![0u8; bs];
        for &blk in txn.get_dirty_blocks() {
            device.read_block(blk, &mut tmp)?;
            let mut data = tmp.clone();
            let mut tag_flags = 0u16;
            if data.len() >= 4
                && u32::from_be_bytes([data[0], data[1], data[2], data[3]]) == JBD2_MAGIC_NUMBER
            {
                data[0..4].copy_from_slice(&0u32.to_be_bytes());
                tag_flags |= TAG_FLAG_ESCAPE;
            }
            per_tag_flags.push(tag_flags);
            journal_data.push(data);
        }

        // May require multiple descriptor blocks.
        let mut chunks = Vec::<(Vec<u8>, usize, usize)>::new(); // (descriptor, start_idx, len)
        let mut idx = 0usize;
        while idx < txn.get_dirty_blocks().len() {
            let mut descriptor = vec![0u8; bs];
            descriptor[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
            descriptor[4..8].copy_from_slice(&JBD2_BLOCKTYPE_DESCRIPTOR.to_be_bytes());
            descriptor[8..12].copy_from_slice(&txn.tid.to_be_bytes());

            let start = idx;
            let mut off = 12usize;
            while idx < txn.get_dirty_blocks().len() {
                let base_flags = per_tag_flags[idx];
                let mut flags = base_flags | if idx > start { TAG_FLAG_SAME_UUID } else { 0 };
                let need = 4
                    + (if has_csum { 2 } else { 0 })
                    + 2
                    + (if has_64bit { 4 } else { 0 })
                    + (if flags & TAG_FLAG_SAME_UUID == 0 {
                        16
                    } else {
                        0
                    });
                if off + need > bs {
                    break;
                }
                if idx + 1 == txn.get_dirty_blocks().len() {
                    flags |= TAG_FLAG_LAST_TAG;
                } else {
                    // Descriptor-local LAST_TAG to terminate parser for this block.
                    let peek_flags = per_tag_flags[idx + 1] | TAG_FLAG_SAME_UUID;
                    let peek_need = 4
                        + (if has_csum { 2 } else { 0 })
                        + 2
                        + (if has_64bit { 4 } else { 0 })
                        + (if peek_flags & TAG_FLAG_SAME_UUID == 0 {
                            16
                        } else {
                            0
                        });
                    if off + need + peek_need > bs {
                        flags |= TAG_FLAG_LAST_TAG;
                    }
                }

                let blk = txn.get_dirty_blocks()[idx];
                descriptor[off..off + 4].copy_from_slice(&(blk as u32).to_be_bytes());
                off += 4;
                if has_csum {
                    let csum16 = (crc32c(0, &journal_data[idx]) & 0xFFFF) as u16;
                    descriptor[off..off + 2].copy_from_slice(&csum16.to_be_bytes());
                    off += 2;
                }
                descriptor[off..off + 2].copy_from_slice(&flags.to_be_bytes());
                off += 2;
                if has_64bit {
                    descriptor[off..off + 4].copy_from_slice(&((blk >> 32) as u32).to_be_bytes());
                    off += 4;
                }
                if flags & TAG_FLAG_SAME_UUID == 0 {
                    descriptor[off..off + 16].copy_from_slice(&journal_sb.s_uuid);
                    off += 16;
                }
                idx += 1;
                if flags & TAG_FLAG_LAST_TAG != 0 {
                    break;
                }
            }
            if idx == start {
                return Err(Ext4Error::CorruptedFs("descriptor cannot fit any tag"));
            }
            chunks.push((descriptor, start, idx - start));
        }

        for (descriptor, data_start, data_len) in &chunks {
            Self::write_journal_block(device, journal_start_block, journal_sb, pos, descriptor)?;
            pos = Self::next_pos(journal_sb, pos);
            for data in journal_data.iter().skip(*data_start).take(*data_len) {
                Self::write_journal_block(device, journal_start_block, journal_sb, pos, data)?;
                pos = Self::next_pos(journal_sb, pos);
            }
        }

        device.flush()?;

        let mut commit_block = vec![0u8; bs];
        commit_block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        commit_block[4..8].copy_from_slice(&JBD2_BLOCKTYPE_COMMIT.to_be_bytes());
        commit_block[8..12].copy_from_slice(&txn.tid.to_be_bytes());
        Self::write_journal_block(device, journal_start_block, journal_sb, pos, &commit_block)?;
        pos = Self::next_pos(journal_sb, pos);
        device.flush()?;

        journal_sb.s_sequence = txn.tid + 1;
        journal_sb.s_start = pos;
        txn.state = TransactionState::Committed;
        Ok(())
    }

    fn next_pos(journal_sb: &JournalSuperBlock, pos: u32) -> u32 {
        let mut p = pos + 1;
        if p >= journal_sb.s_maxlen {
            p = journal_sb.s_first;
        }
        p
    }

    fn write_journal_block<D: BlockDevice>(
        device: &mut D,
        journal_start_block: u64,
        journal_sb: &JournalSuperBlock,
        rel_pos: u32,
        data: &[u8],
    ) -> Result<()> {
        if rel_pos < journal_sb.s_first || rel_pos >= journal_sb.s_maxlen {
            return Err(Ext4Error::CorruptedFs(
                "journal write position out of range",
            ));
        }
        let block_no = journal_start_block + rel_pos as u64;
        device.write_block(block_no, data)
    }
}
