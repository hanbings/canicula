use alloc::vec;

use crate::error::Result;
use crate::fs_core::extent_modifier::ExtentModifier;
use crate::fs_core::extent_walker::ExtentWalker;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_writer::BlockWriter;
use crate::layout::inode::Inode;
use crate::traits::allocator::BlockAllocator;
use crate::traits::block_device::BlockDevice;

/// File data writer.
pub struct FileWriter;

impl FileWriter {
    pub fn write<D: BlockDevice, A: BlockAllocator>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &mut Inode,
        offset: u64,
        data: &[u8],
        block_allocator: &mut A,
    ) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        let block_size = super_block_manager.block_size;
        let mut scratch = vec![0u8; block_size];
        let mut copied = 0usize;
        let mut current_logical = (offset / block_size as u64) as u32;
        let mut offset_in_block = (offset % block_size as u64) as usize;
        let mut prev_physical = 0u64;

        while copied < data.len() {
            let in_this_block = core::cmp::min(block_size - offset_in_block, data.len() - copied);
            let reader = writer.as_reader();
            let mapping = ExtentWalker::logical_to_physical(
                &reader,
                super_block_manager,
                inode,
                current_logical,
            )?;

            let physical = if let Some(m) = mapping {
                m.physical_block
            } else {
                let goal = if prev_physical != 0 {
                    prev_physical + 1
                } else {
                    super_block_manager.super_block.s_first_data_block as u64
                };
                let new_block = block_allocator.alloc_blocks(goal, 1)?[0];
                ExtentModifier::insert_extent(
                    writer,
                    super_block_manager,
                    inode,
                    current_logical,
                    new_block,
                    1,
                    block_allocator,
                )?;
                new_block
            };

            // Partial block writes need read-modify-write.
            if offset_in_block != 0 || in_this_block != block_size {
                writer.device().read_block(physical, &mut scratch)?;
            } else {
                scratch.fill(0);
            }
            scratch[offset_in_block..offset_in_block + in_this_block]
                .copy_from_slice(&data[copied..copied + in_this_block]);
            writer.write_block(physical, &scratch)?;

            prev_physical = physical;
            copied += in_this_block;
            current_logical += 1;
            offset_in_block = 0;
        }

        let end = offset + copied as u64;
        if end > inode.i_size {
            inode.i_size = end;
        }
        inode.i_blocks = Self::compute_i_blocks(writer, super_block_manager, inode)?;
        Ok(copied)
    }

    pub fn truncate<D: BlockDevice, A: BlockAllocator>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &mut Inode,
        new_size: u64,
        block_allocator: &mut A,
    ) -> Result<()> {
        if new_size >= inode.i_size {
            inode.i_size = new_size;
            return Ok(());
        }

        let block_size = super_block_manager.block_size as u64;
        let from_logical = new_size.div_ceil(block_size) as u32;
        let removed = ExtentModifier::remove_extents(
            writer,
            super_block_manager,
            inode,
            from_logical,
            block_allocator,
        )?;

        let mut blocks = vec![];
        for (start, count) in removed {
            for i in 0..count {
                blocks.push(start + i as u64);
            }
        }
        if !blocks.is_empty() {
            block_allocator.free_blocks(&blocks)?;
        }

        // If truncating in the middle of a block, zero the tail.
        let tail_off = (new_size % block_size) as usize;
        if tail_off != 0 {
            let logical = (new_size / block_size) as u32;
            let reader = writer.as_reader();
            if let Some(mapping) =
                ExtentWalker::logical_to_physical(&reader, super_block_manager, inode, logical)?
            {
                let mut buf = vec![0u8; block_size as usize];
                writer
                    .device()
                    .read_block(mapping.physical_block, &mut buf)?;
                buf[tail_off..].fill(0);
                writer.write_block(mapping.physical_block, &buf)?;
            }
        }

        inode.i_size = new_size;
        inode.i_blocks = Self::compute_i_blocks(writer, super_block_manager, inode)?;
        Ok(())
    }

    fn compute_i_blocks<D: BlockDevice>(
        writer: &BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
    ) -> Result<u64> {
        let reader = writer.as_reader();
        let extents = ExtentWalker::walk_all_extents(&reader, super_block_manager, inode)?;
        let total_fs_blocks: u64 = extents.iter().map(|e| e.block_count() as u64).sum();
        Ok(total_fs_blocks * (super_block_manager.block_size as u64 / 512))
    }
}
