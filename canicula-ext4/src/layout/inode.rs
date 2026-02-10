#![allow(dead_code)]

use super::{read_u16_le, read_u32_le};
use crate::error::{Ext4Error, Result};

// Mode constants (i_mode & S_IFMT)
pub const S_IFMT: u16 = 0xF000;
pub const S_IFSOCK: u16 = 0xC000;
pub const S_IFLNK: u16 = 0xA000;
pub const S_IFREG: u16 = 0x8000;
pub const S_IFBLK: u16 = 0x6000;
pub const S_IFDIR: u16 = 0x4000;
pub const S_IFCHR: u16 = 0x2000;
pub const S_IFIFO: u16 = 0x1000;

// Inode flags (i_flags)
pub const EXTENTS_FL: u32 = 0x0008_0000;
pub const INDEX_FL: u32 = 0x0000_1000;
pub const INLINE_FL: u32 = 0x1000_0000;

// FileType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Unknown,
    RegularFile,
    Directory,
    CharDevice,
    BlockDevice,
    Fifo,
    Socket,
    Symlink,
}

// Inode struct

/// Parsed ext4 inode.
///
/// Core metadata for every file, directory, and symlink.
/// uid/gid/size fields are already combined from their lo/hi halves.
#[derive(Debug, Clone)]
pub struct Inode {
    pub i_mode: u16,
    /// Combined: `(uid_hi << 16) | uid_lo`
    pub i_uid: u32,
    /// Combined: `(gid_hi << 16) | gid_lo`
    pub i_gid: u32,
    /// Combined: `(size_hi << 32) | size_lo`
    pub i_size: u64,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_links_count: u16,
    /// Combined from lo + hi, in 512-byte units.
    pub i_blocks: u64,
    pub i_flags: u32,
    /// Raw 60-byte block map / extent tree root.
    pub i_block: [u8; 60],
    pub i_generation: u32,
    /// Combined: `(file_acl_hi << 32) | file_acl_lo`
    pub i_file_acl: u64,
    /// Extra inode size beyond 128 bytes (0 if inode_size <= 128).
    pub i_extra_isize: u16,
    /// Combined: `(checksum_hi << 16) | checksum_lo`
    pub i_checksum: u32,
}

impl Inode {
    /// Parse an inode from raw bytes.
    ///
    /// `raw.len()` must be >= 128 and >= `inode_size`.
    ///
    /// 1. Read the fixed 128-byte base fields.
    /// 2. If `inode_size` > 128, read extended fields (extra_isize, checksum_hi).
    /// 3. Combine uid/gid from lo + hi halves.
    /// 4. Combine size from lo + hi.
    pub fn parse(raw: &[u8], inode_size: u16) -> Result<Inode> {
        if raw.len() < 128 {
            return Err(Ext4Error::CorruptedFs("inode buffer < 128 bytes"));
        }

        // ── Base fields (0x00 .. 0x80) ──────────────────────────────────────

        let i_mode = read_u16_le(raw, 0x00);
        let i_uid_lo = read_u16_le(raw, 0x02);
        let i_size_lo = read_u32_le(raw, 0x04);
        let i_atime = read_u32_le(raw, 0x08);
        let i_ctime = read_u32_le(raw, 0x0C);
        let i_mtime = read_u32_le(raw, 0x10);
        let i_dtime = read_u32_le(raw, 0x14);
        let i_gid_lo = read_u16_le(raw, 0x18);
        let i_links_count = read_u16_le(raw, 0x1A);
        let i_blocks_lo = read_u32_le(raw, 0x1C);
        let i_flags = read_u32_le(raw, 0x20);
        // 0x24: i_osd1 (skipped)

        let mut i_block = [0u8; 60];
        i_block.copy_from_slice(&raw[0x28..0x64]);

        let i_generation = read_u32_le(raw, 0x64);
        let i_file_acl_lo = read_u32_le(raw, 0x68);
        let i_size_hi = read_u32_le(raw, 0x6C);
        // 0x70: i_obso_faddr (skipped)

        // osd2 fields (Linux-specific, 0x74 .. 0x80)

        let i_blocks_hi = read_u16_le(raw, 0x74);
        let i_file_acl_hi = read_u16_le(raw, 0x76);
        let i_uid_hi = read_u16_le(raw, 0x78);
        let i_gid_hi = read_u16_le(raw, 0x7A);
        let i_checksum_lo = read_u16_le(raw, 0x7C);

        // Extended fields (0x80+, if inode_size > 128)

        let (i_extra_isize, i_checksum_hi) = if inode_size > 128 && raw.len() >= 132 {
            (read_u16_le(raw, 0x80), read_u16_le(raw, 0x82))
        } else {
            (0, 0)
        };

        // Combine hi/lo halves

        let i_uid = ((i_uid_hi as u32) << 16) | (i_uid_lo as u32);
        let i_gid = ((i_gid_hi as u32) << 16) | (i_gid_lo as u32);
        let i_size = ((i_size_hi as u64) << 32) | (i_size_lo as u64);
        let i_blocks = ((i_blocks_hi as u64) << 32) | (i_blocks_lo as u64);
        let i_file_acl = ((i_file_acl_hi as u64) << 32) | (i_file_acl_lo as u64);
        let i_checksum = ((i_checksum_hi as u32) << 16) | (i_checksum_lo as u32);

        Ok(Inode {
            i_mode,
            i_uid,
            i_gid,
            i_size,
            i_atime,
            i_ctime,
            i_mtime,
            i_dtime,
            i_links_count,
            i_blocks,
            i_flags,
            i_block,
            i_generation,
            i_file_acl,
            i_extra_isize,
            i_checksum,
        })
    }

    // File type helpers

    /// Determine the file type from `i_mode & S_IFMT`.
    pub fn file_type(&self) -> FileType {
        match self.i_mode & S_IFMT {
            S_IFREG => FileType::RegularFile,
            S_IFDIR => FileType::Directory,
            S_IFLNK => FileType::Symlink,
            S_IFCHR => FileType::CharDevice,
            S_IFBLK => FileType::BlockDevice,
            S_IFIFO => FileType::Fifo,
            S_IFSOCK => FileType::Socket,
            _ => FileType::Unknown,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.i_mode & S_IFMT == S_IFDIR
    }

    pub fn is_file(&self) -> bool {
        self.i_mode & S_IFMT == S_IFREG
    }

    pub fn is_symlink(&self) -> bool {
        self.i_mode & S_IFMT == S_IFLNK
    }

    // Flag helpers

    /// Whether the inode uses an extent tree (vs. indirect block map).
    pub fn uses_extents(&self) -> bool {
        self.i_flags & EXTENTS_FL != 0
    }

    /// Whether the directory uses HTree indexing.
    pub fn uses_htree(&self) -> bool {
        self.i_flags & INDEX_FL != 0
    }

    /// Whether the inode stores data inline (in i_block area).
    pub fn has_inline_data(&self) -> bool {
        self.i_flags & INLINE_FL != 0
    }
}
