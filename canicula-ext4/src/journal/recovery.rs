#![allow(dead_code)]

use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::journal::descriptor::{TAG_FLAG_ESCAPE, parse_descriptor_block};
use crate::journal::jbd2_superblock::{
    JBD2_BLOCKTYPE_COMMIT, JBD2_BLOCKTYPE_DESCRIPTOR, JBD2_BLOCKTYPE_REVOKE, JBD2_MAGIC_NUMBER,
    JournalHeader, JournalSuperBlock,
};
use crate::journal::revoke::parse_revoke_block;
use crate::layout::checksum::crc32c;
use crate::traits::block_device::BlockDevice;

#[derive(Debug, Clone)]
pub struct RecoverySummary {
    pub replayed_transactions: usize,
    pub replayed_blocks: usize,
}

pub struct JournalRecovery;

impl JournalRecovery {
    pub fn needs_recovery(journal_sb: &JournalSuperBlock) -> bool {
        journal_sb.s_start != 0
    }

    pub fn recover<D: BlockDevice>(
        device: &mut D,
        journal_start_block: u64,
        journal_sb: &mut JournalSuperBlock,
        has_64bit: bool,
        has_csum: bool,
    ) -> Result<RecoverySummary> {
        if !Self::needs_recovery(journal_sb) {
            return Ok(RecoverySummary {
                replayed_transactions: 0,
                replayed_blocks: 0,
            });
        }

        let bs = journal_sb.s_blocksize as usize;
        let mut replayed_tx = 0usize;
        let mut replayed_blocks = 0usize;
        let mut pos = journal_sb.s_start;
        let mut expected = journal_sb.s_sequence;
        let mut buf = vec![0u8; bs];

        loop {
            #[derive(Clone, Copy)]
            struct PendingTag {
                block_no: u64,
                data_pos: u32,
                flags: u16,
                checksum: u16,
            }

            let mut pending = Vec::<PendingTag>::new();
            let mut revoked = BTreeSet::<u64>::new();
            let mut scan_pos = pos;
            let mut committed = false;

            loop {
                Self::read_journal_block(device, journal_start_block, scan_pos, &mut buf)?;
                let header = match JournalHeader::parse(&buf) {
                    Ok(h) => h,
                    Err(_) => break,
                };
                if header.h_magic != JBD2_MAGIC_NUMBER || header.h_sequence != expected {
                    break;
                }

                match header.h_blocktype {
                    JBD2_BLOCKTYPE_DESCRIPTOR => {
                        let (_, tags) = parse_descriptor_block(&buf, has_64bit, has_csum)?;
                        let mut data_pos = Self::next_pos(journal_sb, scan_pos);
                        for tag in tags {
                            pending.push(PendingTag {
                                block_no: tag.t_blocknr,
                                data_pos,
                                flags: tag.t_flags,
                                checksum: tag.t_checksum,
                            });
                            data_pos = Self::next_pos(journal_sb, data_pos);
                        }
                        scan_pos = data_pos;
                    }
                    JBD2_BLOCKTYPE_REVOKE => {
                        let (_, revoked_blocks) = parse_revoke_block(&buf, has_64bit)?;
                        for blk in revoked_blocks {
                            revoked.insert(blk);
                        }
                        scan_pos = Self::next_pos(journal_sb, scan_pos);
                    }
                    JBD2_BLOCKTYPE_COMMIT => {
                        committed = true;
                        scan_pos = Self::next_pos(journal_sb, scan_pos);
                        break;
                    }
                    _ => break,
                }
            }

            if !committed {
                break;
            }

            for item in pending {
                if revoked.contains(&item.block_no) {
                    continue;
                }
                let mut block = vec![0u8; bs];
                Self::read_journal_block(device, journal_start_block, item.data_pos, &mut block)?;
                if has_csum {
                    let got = (crc32c(0, &block) & 0xFFFF) as u16;
                    if got != item.checksum {
                        return Err(Ext4Error::InvalidChecksum);
                    }
                }
                if item.flags & TAG_FLAG_ESCAPE != 0 && block.len() >= 4 {
                    block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
                }
                device.write_block(item.block_no, &block)?;
                replayed_blocks += 1;
            }

            replayed_tx += 1;
            expected = expected.wrapping_add(1);
            pos = scan_pos;
        }

        device.flush()?;
        journal_sb.s_start = 0;
        journal_sb.s_sequence = expected;
        Ok(RecoverySummary {
            replayed_transactions: replayed_tx,
            replayed_blocks,
        })
    }

    fn read_journal_block<D: BlockDevice>(
        device: &D,
        journal_start_block: u64,
        rel_pos: u32,
        out: &mut [u8],
    ) -> Result<()> {
        let block_no = journal_start_block + rel_pos as u64;
        device.read_block(block_no, out)
    }

    fn next_pos(journal_sb: &JournalSuperBlock, pos: u32) -> u32 {
        let mut p = pos + 1;
        if p >= journal_sb.s_maxlen {
            p = journal_sb.s_first;
        }
        p
    }
}
