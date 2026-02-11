use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::extent_walker::ExtentWalker;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::dir_entry::DirEntry;
use crate::layout::inode::Inode;
use crate::layout::superblock::INCOMPAT_FILETYPE;
use crate::traits::block_device::BlockDevice;

/// Directory reader for `readdir` and `lookup`.
pub struct DirReader;

impl DirReader {
    /// Read all non-empty directory entries in a directory inode.
    pub fn read_dir_entries<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &Inode,
    ) -> Result<Vec<DirEntry>> {
        if !dir_inode.is_dir() {
            return Err(Ext4Error::CorruptedFs("inode is not a directory"));
        }

        let has_filetype =
            (super_block_manager.super_block.s_feature_incompat & INCOMPAT_FILETYPE) != 0;
        let block_size = super_block_manager.block_size;
        let extents = ExtentWalker::walk_all_extents(reader, super_block_manager, dir_inode)?;
        let mut block_buf = vec![0u8; block_size];
        let mut out = Vec::new();

        for ext in extents {
            if ext.block_count() == 0 {
                continue;
            }
            for i in 0..ext.block_count() {
                if ext.is_uninitialized() {
                    continue;
                }
                reader.read_block(ext.physical_start() + i as u64, &mut block_buf)?;
                let mut off = 0usize;
                while off < block_size {
                    let entry = DirEntry::parse(&block_buf[off..], has_filetype)?;
                    let rec_len = entry.rec_len as usize;
                    if rec_len == 0 {
                        return Err(Ext4Error::CorruptedFs("dir entry rec_len is zero"));
                    }
                    if !entry.is_unused() {
                        out.push(entry);
                    }
                    off += rec_len;
                }
            }
        }

        Ok(out)
    }

    /// Linear lookup in a directory.
    pub fn lookup<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &Inode,
        name: &str,
    ) -> Result<u32> {
        let entries = Self::read_dir_entries(reader, super_block_manager, dir_inode)?;
        for entry in entries {
            if entry.name == name {
                return Ok(entry.inode);
            }
        }
        Err(Ext4Error::NotFound)
    }
}
