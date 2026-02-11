#![allow(dead_code)]

use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::journal::jbd2_superblock::{
    JBD2_BLOCKTYPE_DESCRIPTOR, JBD2_MAGIC_NUMBER, JournalHeader,
};

pub const TAG_FLAG_ESCAPE: u16 = 0x01;
pub const TAG_FLAG_SAME_UUID: u16 = 0x02;
pub const TAG_FLAG_DELETED: u16 = 0x04;
pub const TAG_FLAG_LAST_TAG: u16 = 0x08;

#[derive(Debug, Clone, Copy)]
pub struct JournalTag {
    pub t_blocknr: u64,
    pub t_checksum: u16,
    pub t_flags: u16,
}

pub fn parse_descriptor_block(
    raw: &[u8],
    has_64bit: bool,
    has_csum: bool,
) -> Result<(JournalHeader, Vec<JournalTag>)> {
    let header = JournalHeader::parse(raw)?;
    if header.h_magic != JBD2_MAGIC_NUMBER || header.h_blocktype != JBD2_BLOCKTYPE_DESCRIPTOR {
        return Err(Ext4Error::CorruptedFs("not a descriptor block"));
    }

    let mut tags = Vec::new();
    let mut off = 12usize;
    while off < raw.len() {
        if off + 4 > raw.len() {
            break;
        }
        let blocknr_lo = read_u32_be(raw, off);
        off += 4;

        let checksum = if has_csum {
            if off + 2 > raw.len() {
                return Err(Ext4Error::CorruptedFs("descriptor tag truncated checksum"));
            }
            let c = read_u16_be(raw, off);
            off += 2;
            c
        } else {
            0
        };

        if off + 2 > raw.len() {
            return Err(Ext4Error::CorruptedFs("descriptor tag truncated flags"));
        }
        let flags = read_u16_be(raw, off);
        off += 2;

        let blocknr_hi = if has_64bit {
            if off + 4 > raw.len() {
                return Err(Ext4Error::CorruptedFs(
                    "descriptor tag truncated blocknr_hi",
                ));
            }
            let hi = read_u32_be(raw, off);
            off += 4;
            hi
        } else {
            0
        };

        if flags & TAG_FLAG_SAME_UUID == 0 {
            if off + 16 > raw.len() {
                return Err(Ext4Error::CorruptedFs("descriptor tag truncated uuid"));
            }
            off += 16;
        }

        let tag = JournalTag {
            t_blocknr: ((blocknr_hi as u64) << 32) | blocknr_lo as u64,
            t_checksum: checksum,
            t_flags: flags,
        };
        tags.push(tag);

        if flags & TAG_FLAG_LAST_TAG != 0 {
            break;
        }
    }

    if tags.is_empty() {
        return Err(Ext4Error::CorruptedFs("descriptor has no tags"));
    }
    Ok((header, tags))
}

#[inline]
fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
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
