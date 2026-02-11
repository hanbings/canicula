#![allow(dead_code)]

use crate::error::{Ext4Error, Result};
use crate::layout::{read_u16_le, read_u32_le};

/// Extent tree magic in ext4.
pub const EXTENT_HEADER_MAGIC: u16 = 0xF30A;

/// Parsed extent header (12 bytes).
#[derive(Debug, Clone, Copy)]
pub struct ExtentHeader {
    pub eh_magic: u16,
    pub eh_entries: u16,
    pub eh_max: u16,
    pub eh_depth: u16,
    pub eh_generation: u32,
}

/// Parsed extent index entry (internal node, 12 bytes).
#[derive(Debug, Clone, Copy)]
pub struct ExtentIndex {
    pub ei_block: u32,
    pub ei_leaf_lo: u32,
    pub ei_leaf_hi: u16,
}

/// Parsed extent leaf entry (12 bytes).
#[derive(Debug, Clone, Copy)]
pub struct Extent {
    pub ee_block: u32,
    pub ee_len: u16,
    pub ee_start_hi: u16,
    pub ee_start_lo: u32,
}

impl ExtentHeader {
    /// Parse and validate an extent header.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 12 {
            return Err(Ext4Error::CorruptedFs("extent header too small"));
        }

        let header = ExtentHeader {
            eh_magic: read_u16_le(raw, 0x00),
            eh_entries: read_u16_le(raw, 0x02),
            eh_max: read_u16_le(raw, 0x04),
            eh_depth: read_u16_le(raw, 0x06),
            eh_generation: read_u32_le(raw, 0x08),
        };

        if header.eh_magic != EXTENT_HEADER_MAGIC {
            return Err(Ext4Error::CorruptedFs("invalid extent header magic"));
        }
        if header.eh_entries > header.eh_max {
            return Err(Ext4Error::CorruptedFs("extent header entries > max"));
        }

        Ok(header)
    }
}

impl ExtentIndex {
    /// Parse an extent index entry.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 12 {
            return Err(Ext4Error::CorruptedFs("extent index too small"));
        }
        Ok(ExtentIndex {
            ei_block: read_u32_le(raw, 0x00),
            ei_leaf_lo: read_u32_le(raw, 0x04),
            ei_leaf_hi: read_u16_le(raw, 0x08),
        })
    }

    /// Child extent block physical address.
    pub fn child_block(&self) -> u64 {
        ((self.ei_leaf_hi as u64) << 32) | self.ei_leaf_lo as u64
    }
}

impl Extent {
    /// Parse a leaf extent entry.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 12 {
            return Err(Ext4Error::CorruptedFs("extent leaf too small"));
        }
        Ok(Extent {
            ee_block: read_u32_le(raw, 0x00),
            ee_len: read_u16_le(raw, 0x04),
            ee_start_hi: read_u16_le(raw, 0x06),
            ee_start_lo: read_u32_le(raw, 0x08),
        })
    }

    /// Physical start block this extent maps to.
    pub fn physical_start(&self) -> u64 {
        ((self.ee_start_hi as u64) << 32) | self.ee_start_lo as u64
    }

    /// Number of initialized blocks in this extent.
    pub fn block_count(&self) -> u32 {
        (self.ee_len & 0x7FFF) as u32
    }

    /// Whether this extent is uninitialized (preallocated).
    pub fn is_uninitialized(&self) -> bool {
        self.ee_len & 0x8000 != 0
    }
}
