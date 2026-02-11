#![allow(dead_code)]

use alloc::string::String;

use crate::error::{Ext4Error, Result};

/// ext4 directory entry file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Unknown = 0,
    RegularFile = 1,
    Directory = 2,
    CharDevice = 3,
    BlockDevice = 4,
    Fifo = 5,
    Socket = 6,
    Symlink = 7,
}

impl FileType {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => FileType::RegularFile,
            2 => FileType::Directory,
            3 => FileType::CharDevice,
            4 => FileType::BlockDevice,
            5 => FileType::Fifo,
            6 => FileType::Socket,
            7 => FileType::Symlink,
            _ => FileType::Unknown,
        }
    }
}

/// Parsed ext4 directory entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: FileType,
    pub name: String,
}

impl DirEntry {
    /// Parse a directory entry from the start of `raw`.
    ///
    /// The returned `rec_len` tells caller how far to advance.
    pub fn parse(raw: &[u8], has_filetype: bool) -> Result<Self> {
        if raw.len() < 8 {
            return Err(Ext4Error::CorruptedFs("dir entry too small"));
        }

        let inode = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let rec_len = u16::from_le_bytes([raw[4], raw[5]]);
        let name_len = raw[6];
        let ft_raw = raw[7];

        if rec_len < 8 {
            return Err(Ext4Error::CorruptedFs("dir entry rec_len < 8"));
        }
        if rec_len as usize > raw.len() {
            return Err(Ext4Error::CorruptedFs("dir entry rec_len out of bounds"));
        }
        if rec_len % 4 != 0 {
            return Err(Ext4Error::CorruptedFs("dir entry rec_len not aligned"));
        }
        if 8usize + name_len as usize > rec_len as usize {
            return Err(Ext4Error::CorruptedFs("dir entry name exceeds rec_len"));
        }

        let name_bytes = &raw[8..8 + name_len as usize];
        let name = core::str::from_utf8(name_bytes)
            .map_err(|_| Ext4Error::CorruptedFs("dir entry name is not utf8"))?;

        Ok(DirEntry {
            inode,
            rec_len,
            name_len,
            file_type: if has_filetype {
                FileType::from_u8(ft_raw)
            } else {
                FileType::Unknown
            },
            name: name.into(),
        })
    }

    pub fn is_unused(&self) -> bool {
        self.inode == 0
    }

    pub fn is_dot_or_dotdot(&self) -> bool {
        self.name == "." || self.name == ".."
    }
}
