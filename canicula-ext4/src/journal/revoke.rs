#![allow(dead_code)]

use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::journal::jbd2_superblock::{JBD2_BLOCKTYPE_REVOKE, JBD2_MAGIC_NUMBER, JournalHeader};

pub fn parse_revoke_block(raw: &[u8], has_64bit: bool) -> Result<(JournalHeader, Vec<u64>)> {
    let header = JournalHeader::parse(raw)?;
    if header.h_magic != JBD2_MAGIC_NUMBER || header.h_blocktype != JBD2_BLOCKTYPE_REVOKE {
        return Err(Ext4Error::CorruptedFs("not a revoke block"));
    }
    if raw.len() < 16 {
        return Err(Ext4Error::CorruptedFs("revoke block too small"));
    }

    // r_count is the total byte length of the revoke block (header + entries),
    // NOT the number of entries. See Linux fs/jbd2/recovery.c: scan_revoke_records().
    let r_count = read_u32_be(raw, 12) as usize;
    let entry_size = if has_64bit { 8 } else { 4 };
    if r_count > raw.len() {
        return Err(Ext4Error::CorruptedFs("revoke block r_count exceeds block"));
    }
    if r_count < 16 {
        return Err(Ext4Error::CorruptedFs("revoke block r_count too small"));
    }
    let data_bytes = r_count - 16;
    let num_entries = data_bytes / entry_size;

    let mut out = Vec::with_capacity(num_entries);
    let mut off = 16usize;
    for _ in 0..num_entries {
        let lo = read_u32_be(raw, off) as u64;
        off += 4;
        let blk = if has_64bit {
            let hi = read_u32_be(raw, off) as u64;
            off += 4;
            (hi << 32) | lo
        } else {
            lo
        };
        out.push(blk);
    }
    Ok((header, out))
}

#[inline]
fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
