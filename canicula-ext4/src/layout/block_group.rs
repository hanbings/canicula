#![allow(dead_code)]

use super::{read_u16_le, read_u32_le};
use crate::error::{Ext4Error, Result};
use crate::layout::checksum::block_group_checksum_matches;

/// Parsed ext4 block group descriptor.
///
/// Bridges from the super block to per-group metadata (bitmaps, inode table).
/// 32 bytes in non-64-bit mode, 64 bytes in 64-bit mode.
#[derive(Debug, Clone)]
pub struct BlockGroupDesc {
    // location pointers (lo / hi)
    pub bg_block_bitmap_lo: u32,
    pub bg_block_bitmap_hi: u32,
    pub bg_inode_bitmap_lo: u32,
    pub bg_inode_bitmap_hi: u32,
    pub bg_inode_table_lo: u32,
    pub bg_inode_table_hi: u32,

    // counters (lo / hi)
    pub bg_free_blocks_count_lo: u16,
    pub bg_free_blocks_count_hi: u16,
    pub bg_free_inodes_count_lo: u16,
    pub bg_free_inodes_count_hi: u16,
    pub bg_used_dirs_count_lo: u16,
    pub bg_used_dirs_count_hi: u16,

    // flags & checksum
    pub bg_flags: u16,
    pub bg_checksum: u16,
}

impl BlockGroupDesc {
    /// Parse a block group descriptor from raw bytes.
    ///
    /// - Non-64-bit: reads 32 bytes, hi fields are zero.
    /// - 64-bit: reads 64 bytes for the hi fields.
    pub fn parse(raw: &[u8], is_64bit: bool) -> Result<BlockGroupDesc> {
        if raw.len() < 32 {
            return Err(Ext4Error::CorruptedFs("block group desc too small"));
        }

        if is_64bit && raw.len() < 64 {
            return Err(Ext4Error::CorruptedFs(
                "64-bit block group desc requires >= 64 bytes",
            ));
        }

        let desc = BlockGroupDesc {
            bg_block_bitmap_lo: read_u32_le(raw, 0x00),
            bg_inode_bitmap_lo: read_u32_le(raw, 0x04),
            bg_inode_table_lo: read_u32_le(raw, 0x08),
            bg_free_blocks_count_lo: read_u16_le(raw, 0x0C),
            bg_free_inodes_count_lo: read_u16_le(raw, 0x0E),
            bg_used_dirs_count_lo: read_u16_le(raw, 0x10),
            bg_flags: read_u16_le(raw, 0x12),
            bg_checksum: read_u16_le(raw, 0x1E),

            // 64-bit hi fields
            bg_block_bitmap_hi: if is_64bit { read_u32_le(raw, 0x20) } else { 0 },
            bg_inode_bitmap_hi: if is_64bit { read_u32_le(raw, 0x24) } else { 0 },
            bg_inode_table_hi: if is_64bit { read_u32_le(raw, 0x28) } else { 0 },
            bg_free_blocks_count_hi: if is_64bit { read_u16_le(raw, 0x2C) } else { 0 },
            bg_free_inodes_count_hi: if is_64bit { read_u16_le(raw, 0x2E) } else { 0 },
            bg_used_dirs_count_hi: if is_64bit { read_u16_le(raw, 0x30) } else { 0 },
        };

        Ok(desc)
    }

    // Combined accessors (hi << 32 | lo)

    /// Physical block number of the block bitmap.
    pub fn block_bitmap(&self, is_64bit: bool) -> u64 {
        if is_64bit {
            ((self.bg_block_bitmap_hi as u64) << 32) | (self.bg_block_bitmap_lo as u64)
        } else {
            self.bg_block_bitmap_lo as u64
        }
    }

    /// Physical block number of the inode bitmap.
    pub fn inode_bitmap(&self, is_64bit: bool) -> u64 {
        if is_64bit {
            ((self.bg_inode_bitmap_hi as u64) << 32) | (self.bg_inode_bitmap_lo as u64)
        } else {
            self.bg_inode_bitmap_lo as u64
        }
    }

    /// Physical block number of the inode table.
    pub fn inode_table(&self, is_64bit: bool) -> u64 {
        if is_64bit {
            ((self.bg_inode_table_hi as u64) << 32) | (self.bg_inode_table_lo as u64)
        } else {
            self.bg_inode_table_lo as u64
        }
    }

    /// Free blocks count in this group.
    pub fn free_blocks_count(&self, is_64bit: bool) -> u32 {
        if is_64bit {
            ((self.bg_free_blocks_count_hi as u32) << 16) | (self.bg_free_blocks_count_lo as u32)
        } else {
            self.bg_free_blocks_count_lo as u32
        }
    }

    /// Free inodes count in this group.
    pub fn free_inodes_count(&self, is_64bit: bool) -> u32 {
        if is_64bit {
            ((self.bg_free_inodes_count_hi as u32) << 16) | (self.bg_free_inodes_count_lo as u32)
        } else {
            self.bg_free_inodes_count_lo as u32
        }
    }

    /// Used directory count in this group.
    pub fn used_dirs_count(&self, is_64bit: bool) -> u32 {
        if is_64bit {
            ((self.bg_used_dirs_count_hi as u32) << 16) | (self.bg_used_dirs_count_lo as u32)
        } else {
            self.bg_used_dirs_count_lo as u32
        }
    }

    /// Verify metadata checksum for this descriptor.
    pub fn verify_checksum(&self, csum_seed: u32, group_no: u32, raw_desc: &[u8]) -> Result<()> {
        if !block_group_checksum_matches(csum_seed, group_no, raw_desc, self.bg_checksum) {
            return Err(Ext4Error::InvalidChecksum);
        }
        Ok(())
    }
}
