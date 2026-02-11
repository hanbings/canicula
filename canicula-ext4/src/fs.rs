use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_alloc::block_alloc::BlockGroupAllocState;
use crate::fs_alloc::block_alloc::Ext4BlockAllocator;
use crate::fs_alloc::inode_alloc::Ext4InodeAllocator;
use crate::fs_alloc::inode_alloc::InodeGroupAllocState;
use crate::fs_core::block_group_manager::BlockGroupManager;
use crate::fs_core::dir_reader::DirReader;
use crate::fs_core::dir_writer::DirWriter;
use crate::fs_core::extent_modifier::ExtentModifier;
use crate::fs_core::file_reader::FileReader;
use crate::fs_core::file_writer::FileWriter;
use crate::fs_core::inode_reader::InodeReader;
use crate::fs_core::inode_writer::InodeWriter;
use crate::fs_core::path_resolver::PathResolver;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::io::block_writer::BlockWriter;
use crate::layout::dir_entry::FileType as DirEntryFileType;
use crate::layout::inode::Inode;
use crate::traits::allocator::{BlockAllocator, InodeAllocator};
use crate::traits::block_device::BlockDevice;
use crate::traits::vfs::{FileSystem, InodeOps};

/// Main ext4 filesystem object that wires all modules together.
pub struct Ext4FileSystem<D: BlockDevice> {
    device: D,
    pub sb_manager: SuperBlockManager,
    pub bg_manager: BlockGroupManager,
    pub read_only: bool,
    pub block_allocator: Option<Ext4BlockAllocator>,
    pub inode_allocator: Option<Ext4InodeAllocator>,
}

impl<D: BlockDevice> Ext4FileSystem<D> {
    pub fn mount(device: D, read_only: bool) -> Result<Self> {
        let reader = BlockReader::new(&device);
        let sb_manager = SuperBlockManager::load(&reader)?;
        let bg_manager = BlockGroupManager::load(&reader, &sb_manager)?;

        let (block_allocator, inode_allocator) = if read_only {
            (None, None)
        } else {
            let mut block_groups = Vec::with_capacity(sb_manager.group_count as usize);
            let mut inode_groups = Vec::with_capacity(sb_manager.group_count as usize);
            let mut buf = vec![0u8; sb_manager.block_size];
            let is_64bit = sb_manager.is_64bit;

            let block_bits = sb_manager.super_block.s_blocks_per_group as usize;
            let block_bitmap_bytes = block_bits.div_ceil(8);
            let inode_bits = sb_manager.super_block.s_inodes_per_group as usize;
            let inode_bitmap_bytes = inode_bits.div_ceil(8);

            for g in 0..bg_manager.count() {
                let desc = bg_manager.get_desc(g);

                reader.read_block(bg_manager.block_bitmap_block(g), &mut buf)?;
                block_groups.push(BlockGroupAllocState {
                    block_bitmap: buf[..block_bitmap_bytes].to_vec(),
                    free_blocks_count: desc.free_blocks_count(is_64bit),
                    max_bits: block_bits,
                });

                reader.read_block(bg_manager.inode_bitmap_block(g), &mut buf)?;
                inode_groups.push(InodeGroupAllocState {
                    inode_bitmap: buf[..inode_bitmap_bytes].to_vec(),
                    free_inodes_count: desc.free_inodes_count(is_64bit),
                    free_blocks_count: desc.free_blocks_count(is_64bit),
                    used_dirs_count: desc.used_dirs_count(is_64bit),
                    max_bits: inode_bits,
                });
            }

            (
                Some(Ext4BlockAllocator::new(
                    sb_manager.super_block.s_first_data_block as u64,
                    sb_manager.super_block.s_blocks_per_group,
                    block_groups,
                )),
                Some(Ext4InodeAllocator::new(
                    sb_manager.super_block.s_inodes_per_group,
                    inode_groups,
                )),
            )
        };

        Ok(Self {
            device,
            sb_manager,
            bg_manager,
            read_only,
            block_allocator,
            inode_allocator,
        })
    }

    pub fn resolve_path(&self, path: &str) -> Result<u32> {
        let reader = BlockReader::new(&self.device);
        PathResolver::resolve(&reader, &self.sb_manager, &self.bg_manager, path)
    }

    pub fn resolve_parent(&self, path: &str) -> Result<(u32, alloc::string::String)> {
        let reader = BlockReader::new(&self.device);
        PathResolver::resolve_parent(&reader, &self.sb_manager, &self.bg_manager, path)
    }

    fn read_inode_internal(&self, ino: u32) -> Result<Inode> {
        let reader = BlockReader::new(&self.device);
        InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)
    }
}

impl<D: BlockDevice> FileSystem for Ext4FileSystem<D> {
    fn unmount(&mut self) -> Result<()> {
        self.device.flush()
    }
}

impl<D: BlockDevice> InodeOps for Ext4FileSystem<D> {
    fn lookup(&self, parent: u32, name: &str) -> Result<u32> {
        let reader = BlockReader::new(&self.device);
        let parent_inode =
            InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;
        DirReader::lookup(&reader, &self.sb_manager, &parent_inode, name)
    }

    fn read(&self, ino: u32, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let reader = BlockReader::new(&self.device);
        let inode = InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)?;
        FileReader::read(&reader, &self.sb_manager, &inode, offset, buf)
    }

    fn readdir(&self, ino: u32) -> Result<Vec<crate::layout::dir_entry::DirEntry>> {
        let reader = BlockReader::new(&self.device);
        let inode = InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)?;
        DirReader::read_dir_entries(&reader, &self.sb_manager, &inode)
    }

    fn create(&mut self, parent: u32, name: &str, mode: u16, uid: u32, gid: u32) -> Result<u32> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        if self.lookup(parent, name).is_ok() {
            return Err(Ext4Error::CorruptedFs("entry already exists"));
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<u32> {
            let mut writer = BlockWriter::new(&mut self.device);
            let (new_ino, new_inode) =
                InodeWriter::alloc_and_init_inode(&mut inode_allocator, parent, mode, uid, gid)?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                new_ino,
                &new_inode,
            )?;

            let reader = writer.as_reader();
            let mut parent_inode =
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;
            DirWriter::add_entry(
                &mut writer,
                &self.sb_manager,
                &mut parent_inode,
                name,
                new_ino,
                DirEntryFileType::RegularFile,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;
            Ok(new_ino)
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        result
    }

    fn write(&mut self, ino: u32, offset: u64, data: &[u8]) -> Result<usize> {
        let mut inode = self.read_inode_internal(ino)?;
        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<usize> {
            let mut writer = BlockWriter::new(&mut self.device);
            let n = FileWriter::write(
                &mut writer,
                &self.sb_manager,
                &mut inode,
                offset,
                data,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
            Ok(n)
        })();
        self.block_allocator = Some(block_allocator);
        result
    }

    fn unlink(&mut self, parent: u32, name: &str) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<()> {
            let mut writer = BlockWriter::new(&mut self.device);
            let parent_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?
            };
            let removed_ino =
                DirWriter::remove_entry(&mut writer, &self.sb_manager, &parent_inode, name)?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;

            let mut removed_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, removed_ino)?
            };
            if removed_inode.i_links_count > 0 {
                removed_inode.i_links_count -= 1;
            }
            if removed_inode.i_links_count == 0 {
                let removed = ExtentModifier::remove_extents(
                    &mut writer,
                    &self.sb_manager,
                    &mut removed_inode,
                    0,
                    &mut block_allocator,
                )?;
                let mut pblks = Vec::new();
                for (start, count) in removed {
                    for i in 0..count {
                        pblks.push(start + i as u64);
                    }
                }
                if !pblks.is_empty() {
                    block_allocator.free_blocks(&pblks)?;
                }
                inode_allocator.free_inode(removed_ino)?;
            } else {
                InodeWriter::write_inode(
                    &mut writer,
                    &self.sb_manager,
                    &self.bg_manager,
                    removed_ino,
                    &removed_inode,
                )?;
            }
            Ok(())
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        result
    }
}
