use alloc::string::String;
use alloc::vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::extent_modifier::ExtentModifier;
use crate::fs_core::extent_walker::ExtentWalker;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_writer::BlockWriter;
use crate::layout::dir_entry::FileType;
use crate::layout::inode::Inode;
use crate::traits::allocator::BlockAllocator;
use crate::traits::block_device::BlockDevice;

pub struct DirWriter;

impl DirWriter {
    pub fn add_entry<D: BlockDevice, A: BlockAllocator>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &mut Inode,
        name: &str,
        target_ino: u32,
        file_type: FileType,
        block_allocator: &mut A,
    ) -> Result<()> {
        if !dir_inode.is_dir() {
            return Err(Ext4Error::NotDirectory);
        }
        if name.is_empty() {
            return Err(Ext4Error::CorruptedFs("empty dir entry name"));
        }

        let bs = super_block_manager.block_size;
        let needed = Self::entry_space(name.len());
        let mut block = vec![0u8; bs];
        let blocks = dir_inode.i_size.div_ceil(bs as u64) as u32;

        for logical in 0..blocks {
            let reader = writer.as_reader();
            let Some(mapping) = ExtentWalker::logical_to_physical(
                &reader,
                super_block_manager,
                dir_inode,
                logical,
            )?
            else {
                continue;
            };
            writer
                .device()
                .read_block(mapping.physical_block, &mut block)?;
            let mut off = 0usize;
            while off < bs {
                let inode = Self::read_u32(&block, off);
                let rec_len = Self::read_u16(&block, off + 4) as usize;
                if rec_len == 0 || off + rec_len > bs {
                    return Err(Ext4Error::CorruptedFs(
                        "dir entry rec_len is zero or invalid",
                    ));
                }
                let name_len = block[off + 6] as usize;
                if inode != 0 {
                    let existing = Self::read_name(&block, off, name_len)?;
                    if existing == name {
                        return Err(Ext4Error::CorruptedFs("dir entry already exists"));
                    }
                }

                let actual = if inode == 0 {
                    0
                } else {
                    Self::entry_space(name_len)
                };
                if rec_len >= actual + needed {
                    if inode != 0 {
                        Self::write_u16(&mut block, off + 4, actual as u16);
                    }
                    let new_off = off + actual;
                    Self::write_entry(
                        &mut block,
                        new_off,
                        target_ino,
                        (rec_len - actual) as u16,
                        name,
                        file_type,
                    );
                    writer.write_block(mapping.physical_block, &block)?;
                    return Ok(());
                }
                off += rec_len;
            }
        }

        // No space: allocate a new data block.
        let goal = super_block_manager.super_block.s_first_data_block as u64;
        let new_block = block_allocator.alloc_blocks(goal, 1)?[0];
        let logical = (dir_inode.i_size / bs as u64) as u32;
        ExtentModifier::insert_extent(
            writer,
            super_block_manager,
            dir_inode,
            logical,
            new_block,
            1,
            block_allocator,
        )?;
        block.fill(0);
        Self::write_entry(&mut block, 0, target_ino, bs as u16, name, file_type);
        writer.write_block(new_block, &block)?;
        dir_inode.i_size += bs as u64;
        dir_inode.i_blocks += (bs / 512) as u64;
        Ok(())
    }

    pub fn remove_entry<D: BlockDevice>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &Inode,
        name: &str,
    ) -> Result<u32> {
        if !dir_inode.is_dir() {
            return Err(Ext4Error::NotDirectory);
        }

        let bs = super_block_manager.block_size;
        let mut block = vec![0u8; bs];
        let blocks = dir_inode.i_size.div_ceil(bs as u64) as u32;

        for logical in 0..blocks {
            let reader = writer.as_reader();
            let Some(mapping) = ExtentWalker::logical_to_physical(
                &reader,
                super_block_manager,
                dir_inode,
                logical,
            )?
            else {
                continue;
            };
            writer
                .device()
                .read_block(mapping.physical_block, &mut block)?;
            let mut off = 0usize;
            let mut prev_off = None;
            while off < bs {
                let inode = Self::read_u32(&block, off);
                let rec_len = Self::read_u16(&block, off + 4) as usize;
                if rec_len == 0 || off + rec_len > bs {
                    return Err(Ext4Error::CorruptedFs(
                        "dir entry rec_len is zero or invalid",
                    ));
                }
                let name_len = block[off + 6] as usize;
                if inode != 0 && Self::read_name(&block, off, name_len)? == name {
                    if let Some(prev) = prev_off {
                        let prev_len = Self::read_u16(&block, prev + 4) as usize;
                        Self::write_u16(&mut block, prev + 4, (prev_len + rec_len) as u16);
                    } else {
                        Self::write_u32(&mut block, off, 0);
                    }
                    writer.write_block(mapping.physical_block, &block)?;
                    return Ok(inode);
                }
                if inode != 0 {
                    prev_off = Some(off);
                }
                off += rec_len;
            }
        }

        Err(Ext4Error::NotFound)
    }

    fn entry_space(name_len: usize) -> usize {
        let base = 8 + name_len;
        (base + 3) & !3
    }

    fn write_entry(
        block: &mut [u8],
        off: usize,
        inode: u32,
        rec_len: u16,
        name: &str,
        file_type: FileType,
    ) {
        block[off..off + 4].copy_from_slice(&inode.to_le_bytes());
        block[off + 4..off + 6].copy_from_slice(&rec_len.to_le_bytes());
        block[off + 6] = name.len() as u8;
        block[off + 7] = file_type as u8;
        block[off + 8..off + rec_len as usize].fill(0);
        block[off + 8..off + 8 + name.len()].copy_from_slice(name.as_bytes());
    }

    fn read_name(block: &[u8], off: usize, name_len: usize) -> Result<String> {
        core::str::from_utf8(&block[off + 8..off + 8 + name_len])
            .map(|s| s.into())
            .map_err(|_| Ext4Error::CorruptedFs("dir entry name is not utf8"))
    }

    fn read_u16(buf: &[u8], off: usize) -> u16 {
        u16::from_le_bytes([buf[off], buf[off + 1]])
    }

    fn read_u32(buf: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
    }

    fn write_u16(buf: &mut [u8], off: usize, v: u16) {
        buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn write_u32(buf: &mut [u8], off: usize, v: u32) {
        buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
}
