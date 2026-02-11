use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::block_group::BlockGroupDesc;
use crate::traits::block_device::BlockDevice;

/// Block group manager.
///
/// Loads and caches all block group descriptors from the descriptor table.
/// Provides convenience accessors to locate per-group metadata (bitmaps, inode table).
pub struct BlockGroupManager {
    descriptors: Vec<BlockGroupDesc>,
    is_64bit: bool,
}

impl BlockGroupManager {
    /// Load all block group descriptors from the device.
    ///
    /// 1. Descriptor table starts at block 2 (1 KiB blocks) or block 1 (>= 2 KiB blocks).
    /// 2. Total bytes = `group_count * desc_size`.
    /// 3. Read block-by-block, parse each descriptor.
    pub fn load<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
    ) -> Result<Self> {
        let block_size = super_block_manager.block_size;
        let group_count = super_block_manager.group_count;
        let desc_size = super_block_manager.desc_size as usize;
        let is_64bit = super_block_manager.is_64bit;
        let has_metadata_csum = super_block_manager.has_metadata_csum;

        // Descriptor table start block:
        //   block_size == 1024 → block 2 (block 0 = boot, block 1 = super block)
        //   block_size >= 2048 → block 1 (block 0 contains both boot + super block)
        let desc_table_start = if block_size == 1024 { 2u64 } else { 1u64 };

        let mut descriptors = Vec::with_capacity(group_count as usize);
        let mut descs_parsed = 0u32;

        // Stack-allocated block buffer (supports block sizes up to 4096)
        if block_size > 4096 {
            return Err(Ext4Error::IoError);
        }
        let mut block_buf = [0u8; 4096];

        // Calculate how many blocks the descriptor table spans
        let total_desc_bytes = group_count as usize * desc_size;
        let blocks_needed = (total_desc_bytes + block_size - 1) / block_size;

        for block_idx in 0..blocks_needed as u64 {
            reader.read_block(desc_table_start + block_idx, &mut block_buf[..block_size])?;

            let mut offset = 0;
            while offset + desc_size <= block_size && descs_parsed < group_count {
                let raw_desc = &block_buf[offset..offset + desc_size];
                let desc = BlockGroupDesc::parse(raw_desc, is_64bit)?;
                if has_metadata_csum {
                    desc.verify_checksum(super_block_manager.csum_seed, descs_parsed, raw_desc)?;
                }
                descriptors.push(desc);
                offset += desc_size;
                descs_parsed += 1;
            }
        }

        Ok(BlockGroupManager {
            descriptors,
            is_64bit,
        })
    }

    /// Get the descriptor for the given block group number.
    pub fn get_desc(&self, group_no: u32) -> &BlockGroupDesc {
        &self.descriptors[group_no as usize]
    }

    /// Physical block number of the inode table for the given group.
    pub fn inode_table_block(&self, group_no: u32) -> u64 {
        self.descriptors[group_no as usize].inode_table(self.is_64bit)
    }

    /// Physical block number of the block bitmap for the given group.
    pub fn block_bitmap_block(&self, group_no: u32) -> u64 {
        self.descriptors[group_no as usize].block_bitmap(self.is_64bit)
    }

    /// Physical block number of the inode bitmap for the given group.
    pub fn inode_bitmap_block(&self, group_no: u32) -> u64 {
        self.descriptors[group_no as usize].inode_bitmap(self.is_64bit)
    }

    /// Number of loaded descriptors.
    pub fn count(&self) -> u32 {
        self.descriptors.len() as u32
    }

    /// Get a mutable reference to a descriptor (for updating free counts).
    pub fn get_desc_mut(&mut self, group_no: u32) -> &mut BlockGroupDesc {
        &mut self.descriptors[group_no as usize]
    }

    /// Descriptor table start block.
    pub fn desc_table_start(block_size: usize) -> u64 {
        if block_size == 1024 { 2u64 } else { 1u64 }
    }
}
