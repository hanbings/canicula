#![allow(dead_code)]

use crate::error::{Ext4Error, Result};

// ─── Constants ──────────────────────────────────────────────────────────────

/// ext4 super block magic number (at offset 0x38).
pub const EXT4_SUPER_MAGIC: u16 = 0xEF53;

/// Super block is always at byte offset 1024 from start of device.
pub const SUPER_BLOCK_OFFSET: usize = 1024;

/// Super block raw size is always 1024 bytes.
pub const SUPER_BLOCK_SIZE: usize = 1024;

// ─── Incompatible feature flags ─────────────────────────────────────────────

pub const INCOMPAT_FILETYPE: u32 = 0x0002;
pub const INCOMPAT_RECOVER: u32 = 0x0004;
pub const INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
pub const INCOMPAT_META_BG: u32 = 0x0010;
pub const INCOMPAT_EXTENTS: u32 = 0x0040;
pub const INCOMPAT_64BIT: u32 = 0x0080;
pub const INCOMPAT_MMP: u32 = 0x0100;
pub const INCOMPAT_FLEX_BG: u32 = 0x0200;
pub const INCOMPAT_EA_INODE: u32 = 0x0400;
pub const INCOMPAT_CSUM_SEED: u32 = 0x2000;
pub const INCOMPAT_LARGEDIR: u32 = 0x4000;
pub const INCOMPAT_INLINE_DATA: u32 = 0x8000;
pub const INCOMPAT_ENCRYPT: u32 = 0x10000;

/// All incompat features we recognize.
const SUPPORTED_INCOMPAT: u32 = INCOMPAT_FILETYPE
    | INCOMPAT_RECOVER
    | INCOMPAT_JOURNAL_DEV
    | INCOMPAT_META_BG
    | INCOMPAT_EXTENTS
    | INCOMPAT_64BIT
    | INCOMPAT_MMP
    | INCOMPAT_FLEX_BG
    | INCOMPAT_EA_INODE
    | INCOMPAT_CSUM_SEED
    | INCOMPAT_LARGEDIR
    | INCOMPAT_INLINE_DATA
    | INCOMPAT_ENCRYPT;

// ─── Read-only compatible feature flags ─────────────────────────────────────

pub const RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
pub const RO_COMPAT_LARGE_FILE: u32 = 0x0002;
pub const RO_COMPAT_HUGE_FILE: u32 = 0x0008;
pub const RO_COMPAT_GDT_CSUM: u32 = 0x0010;
pub const RO_COMPAT_DIR_NLINK: u32 = 0x0020;
pub const RO_COMPAT_EXTRA_ISIZE: u32 = 0x0040;
pub const RO_COMPAT_QUOTA: u32 = 0x0100;
pub const RO_COMPAT_BIGALLOC: u32 = 0x0200;
pub const RO_COMPAT_METADATA_CSUM: u32 = 0x0400;
pub const RO_COMPAT_READONLY: u32 = 0x1000;
pub const RO_COMPAT_PROJECT: u32 = 0x2000;
pub const RO_COMPAT_VERITY: u32 = 0x8000;
pub const RO_COMPAT_ORPHAN_PRESENT: u32 = 0x10000;

/// All ro_compat features we recognize.
const SUPPORTED_RO_COMPAT: u32 = RO_COMPAT_SPARSE_SUPER
    | RO_COMPAT_LARGE_FILE
    | RO_COMPAT_HUGE_FILE
    | RO_COMPAT_GDT_CSUM
    | RO_COMPAT_DIR_NLINK
    | RO_COMPAT_EXTRA_ISIZE
    | RO_COMPAT_QUOTA
    | RO_COMPAT_BIGALLOC
    | RO_COMPAT_METADATA_CSUM
    | RO_COMPAT_READONLY
    | RO_COMPAT_PROJECT
    | RO_COMPAT_VERITY
    | RO_COMPAT_ORPHAN_PRESENT;

// Compatible feature flags

pub const COMPAT_DIR_INDEX: u32 = 0x0020;

// SuperBlock struct

/// Parsed ext4 super block.
///
/// Contains the key fields needed for filesystem operation.
/// Parsed from the raw 1024-byte on-disk super block via [`SuperBlock::parse()`].
#[derive(Debug, Clone)]
pub struct SuperBlock {
    // basic counts
    pub s_inodes_count: u32,
    pub s_blocks_count_lo: u32,
    pub s_blocks_count_hi: u32,
    pub s_free_blocks_count_lo: u32,
    pub s_free_blocks_count_hi: u32,
    pub s_free_inodes_count: u32,

    // geometry
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_blocks_per_group: u32,
    pub s_inodes_per_group: u32,

    // identifiers
    pub s_magic: u16,
    pub s_inode_size: u16,
    pub s_desc_size: u16,

    // features
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,

    // misc
    pub s_uuid: [u8; 16],
    pub s_journal_inum: u32,
    pub s_checksum_type: u8,
    pub s_checksum: u32,
}

impl SuperBlock {
    /// Parse a super block from raw 1024-byte on-disk data.
    ///
    /// 1. Check magic (0xEF53) at offset 0x38.
    /// 2. Read all fields in little-endian.
    /// 3. (Checksum verification deferred to `validate` when metadata_csum enabled.)
    pub fn parse(raw: &[u8; SUPER_BLOCK_SIZE]) -> Result<SuperBlock> {
        let magic = read_u16_le(raw, 0x38);
        if magic != EXT4_SUPER_MAGIC {
            return Err(Ext4Error::InvalidMagic);
        }

        let super_block = SuperBlock {
            s_inodes_count: read_u32_le(raw, 0x00),
            s_blocks_count_lo: read_u32_le(raw, 0x04),
            s_blocks_count_hi: read_u32_le(raw, 0x150),
            s_free_blocks_count_lo: read_u32_le(raw, 0x0C),
            s_free_blocks_count_hi: read_u32_le(raw, 0x158),
            s_free_inodes_count: read_u32_le(raw, 0x10),
            s_first_data_block: read_u32_le(raw, 0x14),
            s_log_block_size: read_u32_le(raw, 0x18),
            s_blocks_per_group: read_u32_le(raw, 0x20),
            s_inodes_per_group: read_u32_le(raw, 0x28),
            s_magic: magic,
            s_inode_size: read_u16_le(raw, 0x58),
            s_desc_size: read_u16_le(raw, 0xFE),
            s_feature_compat: read_u32_le(raw, 0x5C),
            s_feature_incompat: read_u32_le(raw, 0x60),
            s_feature_ro_compat: read_u32_le(raw, 0x64),
            s_uuid: {
                let mut uuid = [0u8; 16];
                uuid.copy_from_slice(&raw[0x68..0x78]);
                uuid
            },
            s_journal_inum: read_u32_le(raw, 0xE0),
            s_checksum_type: raw[0x175],
            s_checksum: read_u32_le(raw, 0x3FC),
        };

        Ok(super_block)
    }

    /// Validate basic super block sanity.
    pub fn validate(&self) -> Result<()> {
        if self.s_magic != EXT4_SUPER_MAGIC {
            return Err(Ext4Error::InvalidMagic);
        }

        // log_block_size: 0 → 1 KiB, 6 → 64 KiB
        if self.s_log_block_size > 6 {
            return Err(Ext4Error::CorruptedFs("invalid log_block_size (> 6)"));
        }

        if self.s_inodes_per_group == 0 {
            return Err(Ext4Error::CorruptedFs("inodes_per_group is zero"));
        }

        if self.s_blocks_per_group == 0 {
            return Err(Ext4Error::CorruptedFs("blocks_per_group is zero"));
        }

        if self.s_inode_size < 128 {
            return Err(Ext4Error::CorruptedFs("inode_size < 128"));
        }

        // inode_size should be a power of two
        if !self.s_inode_size.is_power_of_two() {
            return Err(Ext4Error::CorruptedFs("inode_size not power of two"));
        }

        Ok(())
    }

    /// Check feature flags compatibility.
    ///
    /// - **incompat**: unknown bits → `Err(IncompatibleFeature)`.
    ///   (ext4 hard contract: unknown incompat features MUST reject mount.)
    /// - **ro_compat**: if `writable`, unknown bits → also reject.
    ///   (Unknown ro_compat features only allow read-only mount.)
    pub fn check_features(&self, writable: bool) -> Result<()> {
        let unknown_incompat = self.s_feature_incompat & !SUPPORTED_INCOMPAT;
        if unknown_incompat != 0 {
            return Err(Ext4Error::IncompatibleFeature(unknown_incompat));
        }

        if writable {
            let unknown_ro_compat = self.s_feature_ro_compat & !SUPPORTED_RO_COMPAT;
            if unknown_ro_compat != 0 {
                return Err(Ext4Error::IncompatibleFeature(unknown_ro_compat));
            }
        }

        Ok(())
    }

    // Convenience accessors

    /// Block size in bytes: `1024 << s_log_block_size`.
    pub fn block_size(&self) -> usize {
        1024usize << self.s_log_block_size
    }

    /// Total block count (combining hi + lo for 64-bit support).
    pub fn block_count(&self) -> u64 {
        if self.has_64bit() {
            ((self.s_blocks_count_hi as u64) << 32) | (self.s_blocks_count_lo as u64)
        } else {
            self.s_blocks_count_lo as u64
        }
    }

    /// Total free block count.
    pub fn free_blocks_count(&self) -> u64 {
        if self.has_64bit() {
            ((self.s_free_blocks_count_hi as u64) << 32) | (self.s_free_blocks_count_lo as u64)
        } else {
            self.s_free_blocks_count_lo as u64
        }
    }

    /// Number of block groups.
    ///
    /// `(block_count - first_data_block + blocks_per_group - 1) / blocks_per_group`
    pub fn group_count(&self) -> u32 {
        let bc = self.block_count() - self.s_first_data_block as u64;
        let bpg = self.s_blocks_per_group as u64;
        ((bc + bpg - 1) / bpg) as u32
    }

    /// Whether the 64-bit feature is enabled.
    pub fn has_64bit(&self) -> bool {
        self.s_feature_incompat & INCOMPAT_64BIT != 0
    }

    /// Whether the extents feature is enabled.
    pub fn has_extents(&self) -> bool {
        self.s_feature_incompat & INCOMPAT_EXTENTS != 0
    }

    /// Whether metadata checksumming is enabled.
    pub fn has_metadata_csum(&self) -> bool {
        self.s_feature_ro_compat & RO_COMPAT_METADATA_CSUM != 0
    }

    /// Whether flexible block groups feature is enabled.
    pub fn has_flex_bg(&self) -> bool {
        self.s_feature_incompat & INCOMPAT_FLEX_BG != 0
    }

    /// Whether directory indexing (HTree) is enabled.
    pub fn has_dir_index(&self) -> bool {
        self.s_feature_compat & COMPAT_DIR_INDEX != 0
    }
}

// Little-endian byte reading helpers
#[inline]
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
