use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::extent::{Extent, ExtentHeader, ExtentIndex};
use crate::layout::inode::Inode;
use crate::traits::block_device::BlockDevice;

/// Mapping result for one logical block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalMapping {
    pub physical_block: u64,
    pub length: u32,
    pub uninitialized: bool,
}

/// Extent walker.
///
/// Traverses ext4 extent trees to convert logical file blocks into physical blocks.
pub struct ExtentWalker;

impl ExtentWalker {
    /// Translate a logical block number to physical mapping.
    ///
    /// Returns `Ok(None)` for sparse holes.
    pub fn logical_to_physical<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
        logical_block: u32,
    ) -> Result<Option<PhysicalMapping>> {
        if !inode.uses_extents() {
            return Err(Ext4Error::CorruptedFs("inode does not use extents"));
        }

        let header = ExtentHeader::parse(&inode.i_block[..12])?;
        let mut buf = vec![0u8; super_block_manager.block_size];
        Self::logical_to_physical_in_node(reader, logical_block, header, &inode.i_block, &mut buf)
    }

    /// Walk all leaf extents in the inode tree.
    pub fn walk_all_extents<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
    ) -> Result<Vec<Extent>> {
        if !inode.uses_extents() {
            return Err(Ext4Error::CorruptedFs("inode does not use extents"));
        }

        let header = ExtentHeader::parse(&inode.i_block[..12])?;
        let mut out = Vec::new();
        let mut buf = vec![0u8; super_block_manager.block_size];
        Self::walk_all_in_node(reader, header, &inode.i_block, &mut buf, &mut out)?;
        Ok(out)
    }

    fn logical_to_physical_in_node<D: BlockDevice>(
        reader: &BlockReader<D>,
        logical_block: u32,
        header: ExtentHeader,
        node_bytes: &[u8],
        scratch: &mut [u8],
    ) -> Result<Option<PhysicalMapping>> {
        let entries = header.eh_entries as usize;
        let table_bytes = 12 + entries * 12;
        if node_bytes.len() < table_bytes {
            return Err(Ext4Error::CorruptedFs("extent node truncated"));
        }

        if header.eh_depth == 0 {
            for idx in 0..entries {
                let off = 12 + idx * 12;
                let ext = Extent::parse(&node_bytes[off..off + 12])?;
                let count = ext.block_count();
                if count == 0 {
                    continue;
                }
                if logical_block >= ext.ee_block && logical_block < ext.ee_block + count {
                    let delta = logical_block - ext.ee_block;
                    return Ok(Some(PhysicalMapping {
                        physical_block: ext.physical_start() + delta as u64,
                        length: count - delta,
                        uninitialized: ext.is_uninitialized(),
                    }));
                }
            }
            return Ok(None);
        }

        let mut selected: Option<ExtentIndex> = None;
        for idx in 0..entries {
            let off = 12 + idx * 12;
            let item = ExtentIndex::parse(&node_bytes[off..off + 12])?;
            if item.ei_block <= logical_block {
                selected = Some(item);
            } else {
                break;
            }
        }

        let child = match selected {
            Some(v) => v,
            None => return Ok(None),
        };

        reader.read_block(child.child_block(), scratch)?;
        let child_bytes = scratch.to_vec();
        let child_header = ExtentHeader::parse(&child_bytes[..12])?;
        if child_header.eh_depth + 1 != header.eh_depth {
            return Err(Ext4Error::CorruptedFs("extent tree depth mismatch"));
        }
        Self::logical_to_physical_in_node(
            reader,
            logical_block,
            child_header,
            &child_bytes,
            scratch,
        )
    }

    fn walk_all_in_node<D: BlockDevice>(
        reader: &BlockReader<D>,
        header: ExtentHeader,
        node_bytes: &[u8],
        scratch: &mut [u8],
        out: &mut Vec<Extent>,
    ) -> Result<()> {
        let entries = header.eh_entries as usize;
        let table_bytes = 12 + entries * 12;
        if node_bytes.len() < table_bytes {
            return Err(Ext4Error::CorruptedFs("extent node truncated"));
        }

        if header.eh_depth == 0 {
            for idx in 0..entries {
                let off = 12 + idx * 12;
                out.push(Extent::parse(&node_bytes[off..off + 12])?);
            }
            return Ok(());
        }

        for idx in 0..entries {
            let off = 12 + idx * 12;
            let item = ExtentIndex::parse(&node_bytes[off..off + 12])?;
            reader.read_block(item.child_block(), scratch)?;
            let child_bytes = scratch.to_vec();
            let child_header = ExtentHeader::parse(&child_bytes[..12])?;
            if child_header.eh_depth + 1 != header.eh_depth {
                return Err(Ext4Error::CorruptedFs("extent tree depth mismatch"));
            }
            Self::walk_all_in_node(reader, child_header, &child_bytes, scratch, out)?;
        }
        Ok(())
    }
}
