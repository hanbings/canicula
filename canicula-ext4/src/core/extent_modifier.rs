use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::extent_walker::ExtentWalker;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_writer::BlockWriter;
use crate::layout::extent::{EXTENT_HEADER_MAGIC, Extent, ExtentHeader, ExtentIndex};
use crate::layout::inode::Inode;
use crate::traits::allocator::BlockAllocator;
use crate::traits::block_device::BlockDevice;

/// Extent tree modifier.
///
pub struct ExtentModifier;

#[derive(Clone, Copy)]
struct NodeRef {
    first_logical: u32,
    block_no: u64,
}

impl ExtentModifier {
    pub fn insert_extent<D: BlockDevice, A: BlockAllocator>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &mut Inode,
        logical_block: u32,
        physical_block: u64,
        count: u16,
        block_allocator: &mut A,
    ) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        if !inode.uses_extents() {
            return Err(Ext4Error::CorruptedFs("inode does not use extents"));
        }

        if count > 0x7FFF {
            return Err(Ext4Error::CorruptedFs("extent length overflow"));
        }

        let mut extents = Self::collect_extents(writer, super_block_manager, inode)?;
        extents.push(Extent {
            ee_block: logical_block,
            ee_len: count,
            ee_start_hi: (physical_block >> 32) as u16,
            ee_start_lo: physical_block as u32,
        });
        let normalized = Self::normalize_extents(extents)?;
        Self::rebuild_tree(
            writer,
            super_block_manager,
            inode,
            &normalized,
            block_allocator,
        )
    }

    pub fn remove_extents<D: BlockDevice, A: BlockAllocator>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &mut Inode,
        from_logical: u32,
        block_allocator: &mut A,
    ) -> Result<Vec<(u64, u32)>> {
        if !inode.uses_extents() {
            return Err(Ext4Error::CorruptedFs("inode does not use extents"));
        }
        let extents = Self::collect_extents(writer, super_block_manager, inode)?;
        let mut kept = Vec::new();
        let mut removed = Vec::new();

        for mut ext in extents {
            let cnt = ext.block_count();
            if cnt == 0 {
                continue;
            }
            let start = ext.ee_block;
            let end = start + cnt;
            if start >= from_logical {
                removed.push((ext.physical_start(), cnt));
                continue;
            }
            if end > from_logical {
                // Truncate partially overlapped extent.
                let keep_len = from_logical - start;
                let remove_len = cnt - keep_len;
                removed.push((ext.physical_start() + keep_len as u64, remove_len));
                ext.ee_len = keep_len as u16;
            }
            kept.push(ext);
        }

        let normalized = Self::normalize_extents(kept)?;
        Self::rebuild_tree(
            writer,
            super_block_manager,
            inode,
            &normalized,
            block_allocator,
        )?;
        Ok(removed)
    }

    fn collect_extents<D: BlockDevice>(
        writer: &BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
    ) -> Result<Vec<Extent>> {
        let reader = writer.as_reader();
        let mut extents = ExtentWalker::walk_all_extents(&reader, super_block_manager, inode)?;
        extents.sort_by_key(|e| e.ee_block);
        Ok(extents)
    }

    fn normalize_extents(mut extents: Vec<Extent>) -> Result<Vec<Extent>> {
        if extents.is_empty() {
            return Ok(extents);
        }

        extents.sort_by_key(|e| e.ee_block);
        let mut out = Vec::with_capacity(extents.len());
        out.push(extents[0]);

        for ext in extents.iter().skip(1) {
            let last = out.last_mut().expect("out is not empty");
            let last_cnt = last.block_count();
            let last_end = last.ee_block + last_cnt;
            let last_pend = last.physical_start() + last_cnt as u64;
            let cur_cnt = ext.block_count();

            if ext.ee_block < last_end {
                return Err(Ext4Error::CorruptedFs("overlapping extents"));
            }

            if ext.ee_block == last_end && ext.physical_start() == last_pend {
                let merged = last_cnt + cur_cnt;
                if merged > 0x7FFF {
                    return Err(Ext4Error::CorruptedFs("extent length overflow"));
                }
                last.ee_len = merged as u16;
            } else {
                out.push(*ext);
            }
        }
        Ok(out)
    }

    fn rebuild_tree<D: BlockDevice, A: BlockAllocator>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &mut Inode,
        extents: &[Extent],
        block_allocator: &mut A,
    ) -> Result<()> {
        let mut old_tree_blocks = Self::collect_tree_blocks(writer, super_block_manager, inode)?;
        old_tree_blocks.sort_unstable();
        old_tree_blocks.dedup();

        let block_size = super_block_manager.block_size;
        let node_cap = (block_size - 12) / 12;
        if node_cap == 0 {
            return Err(Ext4Error::CorruptedFs("invalid extent node capacity"));
        }

        // Fast path: fits into inode root leaf directly.
        if extents.len() <= 4 {
            let header = ExtentHeader {
                eh_magic: EXTENT_HEADER_MAGIC,
                eh_entries: extents.len() as u16,
                eh_max: 4,
                eh_depth: 0,
                eh_generation: 0,
            };
            Self::write_header(&mut inode.i_block, &header);
            Self::write_leaf_extents(&mut inode.i_block, extents, extents.len());
            if !old_tree_blocks.is_empty() {
                block_allocator.free_blocks(&old_tree_blocks)?;
            }
            return Ok(());
        }

        let mut level = Vec::<NodeRef>::new();

        // Build leaf level (depth=0) into data blocks.
        for chunk in extents.chunks(node_cap) {
            let blk = block_allocator
                .alloc_blocks(super_block_manager.super_block.s_first_data_block as u64, 1)?[0];
            let mut buf = vec![0u8; block_size];
            let header = ExtentHeader {
                eh_magic: EXTENT_HEADER_MAGIC,
                eh_entries: chunk.len() as u16,
                eh_max: node_cap as u16,
                eh_depth: 0,
                eh_generation: 0,
            };
            Self::write_header_slice(&mut buf, &header);
            Self::write_leaf_entries_slice(&mut buf, chunk);
            writer.write_block(blk, &buf)?;
            level.push(NodeRef {
                first_logical: chunk[0].ee_block,
                block_no: blk,
            });
        }

        let mut depth = 1u16;
        while level.len() > 4 {
            let mut next = Vec::<NodeRef>::new();
            for chunk in level.chunks(node_cap) {
                let blk = block_allocator
                    .alloc_blocks(super_block_manager.super_block.s_first_data_block as u64, 1)?[0];
                let mut buf = vec![0u8; block_size];
                let header = ExtentHeader {
                    eh_magic: EXTENT_HEADER_MAGIC,
                    eh_entries: chunk.len() as u16,
                    eh_max: node_cap as u16,
                    eh_depth: depth,
                    eh_generation: 0,
                };
                Self::write_header_slice(&mut buf, &header);
                Self::write_index_entries_slice(&mut buf, chunk);
                writer.write_block(blk, &buf)?;
                next.push(NodeRef {
                    first_logical: chunk[0].first_logical,
                    block_no: blk,
                });
            }
            level = next;
            depth += 1;
        }

        // Final root (in inode) as index node.
        let root_header = ExtentHeader {
            eh_magic: EXTENT_HEADER_MAGIC,
            eh_entries: level.len() as u16,
            eh_max: 4,
            eh_depth: depth,
            eh_generation: 0,
        };
        Self::write_header(&mut inode.i_block, &root_header);
        Self::write_index_entries_inode(&mut inode.i_block, &level);
        if !old_tree_blocks.is_empty() {
            block_allocator.free_blocks(&old_tree_blocks)?;
        }
        Ok(())
    }

    fn collect_tree_blocks<D: BlockDevice>(
        writer: &BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
    ) -> Result<Vec<u64>> {
        let root = ExtentHeader::parse(&inode.i_block[..12])?;
        if root.eh_depth == 0 {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        let reader = writer.as_reader();
        let mut scratch = vec![0u8; super_block_manager.block_size];
        Self::collect_tree_blocks_in_node(&reader, root, &inode.i_block, &mut scratch, &mut out)?;
        Ok(out)
    }

    fn collect_tree_blocks_in_node<D: BlockDevice>(
        reader: &crate::io::block_reader::BlockReader<&D>,
        header: ExtentHeader,
        node_bytes: &[u8],
        scratch: &mut [u8],
        out: &mut Vec<u64>,
    ) -> Result<()> {
        if header.eh_depth == 0 {
            return Ok(());
        }
        let entries = header.eh_entries as usize;
        let table_bytes = 12 + entries * 12;
        if node_bytes.len() < table_bytes {
            return Err(Ext4Error::CorruptedFs("extent node truncated"));
        }

        for idx in 0..entries {
            let off = 12 + idx * 12;
            let item = ExtentIndex::parse(&node_bytes[off..off + 12])?;
            let child = item.child_block();
            out.push(child);
            reader.read_block(child, scratch)?;
            let child_bytes = scratch.to_vec();
            let child_header = ExtentHeader::parse(&child_bytes[..12])?;
            if child_header.eh_depth + 1 != header.eh_depth {
                return Err(Ext4Error::CorruptedFs("extent tree depth mismatch"));
            }
            Self::collect_tree_blocks_in_node(reader, child_header, &child_bytes, scratch, out)?;
        }
        Ok(())
    }

    fn write_header_slice(buf: &mut [u8], header: &ExtentHeader) {
        buf[0..2].copy_from_slice(&header.eh_magic.to_le_bytes());
        buf[2..4].copy_from_slice(&header.eh_entries.to_le_bytes());
        buf[4..6].copy_from_slice(&header.eh_max.to_le_bytes());
        buf[6..8].copy_from_slice(&header.eh_depth.to_le_bytes());
        buf[8..12].copy_from_slice(&header.eh_generation.to_le_bytes());
    }

    fn write_leaf_entries_slice(buf: &mut [u8], extents: &[Extent]) {
        for (i, ext) in extents.iter().enumerate() {
            let off = 12 + i * 12;
            buf[off..off + 4].copy_from_slice(&ext.ee_block.to_le_bytes());
            buf[off + 4..off + 6].copy_from_slice(&ext.ee_len.to_le_bytes());
            buf[off + 6..off + 8].copy_from_slice(&ext.ee_start_hi.to_le_bytes());
            buf[off + 8..off + 12].copy_from_slice(&ext.ee_start_lo.to_le_bytes());
        }
    }

    fn write_index_entries_slice(buf: &mut [u8], children: &[NodeRef]) {
        for (i, child) in children.iter().enumerate() {
            let off = 12 + i * 12;
            buf[off..off + 4].copy_from_slice(&child.first_logical.to_le_bytes());
            buf[off + 4..off + 8].copy_from_slice(&(child.block_no as u32).to_le_bytes());
            buf[off + 8..off + 10].copy_from_slice(&((child.block_no >> 32) as u16).to_le_bytes());
            buf[off + 10..off + 12].fill(0);
        }
    }

    fn write_index_entries_inode(buf: &mut [u8; 60], children: &[NodeRef]) {
        buf[12..].fill(0);
        for (i, child) in children.iter().enumerate() {
            let off = 12 + i * 12;
            buf[off..off + 4].copy_from_slice(&child.first_logical.to_le_bytes());
            buf[off + 4..off + 8].copy_from_slice(&(child.block_no as u32).to_le_bytes());
            buf[off + 8..off + 10].copy_from_slice(&((child.block_no >> 32) as u16).to_le_bytes());
            buf[off + 10..off + 12].fill(0);
        }
    }

    fn write_header(buf: &mut [u8; 60], header: &ExtentHeader) {
        buf[0..2].copy_from_slice(&header.eh_magic.to_le_bytes());
        buf[2..4].copy_from_slice(&header.eh_entries.to_le_bytes());
        buf[4..6].copy_from_slice(&header.eh_max.to_le_bytes());
        buf[6..8].copy_from_slice(&header.eh_depth.to_le_bytes());
        buf[8..12].copy_from_slice(&header.eh_generation.to_le_bytes());
    }

    fn write_leaf_extents(buf: &mut [u8; 60], extents: &[Extent], used: usize) {
        let table_bytes = 12 + used * 12;
        buf[12..].fill(0);
        for (i, ext) in extents.iter().enumerate() {
            let off = 12 + i * 12;
            buf[off..off + 4].copy_from_slice(&ext.ee_block.to_le_bytes());
            buf[off + 4..off + 6].copy_from_slice(&ext.ee_len.to_le_bytes());
            buf[off + 6..off + 8].copy_from_slice(&ext.ee_start_hi.to_le_bytes());
            buf[off + 8..off + 12].copy_from_slice(&ext.ee_start_lo.to_le_bytes());
        }
        if table_bytes < buf.len() {
            buf[table_bytes..].fill(0);
        }
    }

    pub fn init_empty_extent_root(inode: &mut Inode) {
        inode.i_block.fill(0);
        inode.i_block[0..2].copy_from_slice(&EXTENT_HEADER_MAGIC.to_le_bytes());
        inode.i_block[2..4].copy_from_slice(&0u16.to_le_bytes());
        inode.i_block[4..6].copy_from_slice(&4u16.to_le_bytes());
        inode.i_block[6..8].copy_from_slice(&0u16.to_le_bytes());
        inode.i_block[8..12].copy_from_slice(&0u32.to_le_bytes());
    }
}
