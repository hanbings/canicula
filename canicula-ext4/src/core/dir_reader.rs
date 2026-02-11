use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::extent_walker::ExtentWalker;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::dir_entry::DirEntry;
use crate::layout::htree::{DxNode, DxRoot, compute_hash, find_candidate_blocks};
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

    /// Lookup a name in a directory. Uses HTree if available, falls back to linear scan.
    pub fn lookup<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &Inode,
        name: &str,
    ) -> Result<u32> {
        if dir_inode.uses_htree() {
            match Self::htree_lookup(reader, super_block_manager, dir_inode, name) {
                Ok(v) => return Ok(v),
                // Fall back to linear scan on any htree failure
                Err(Ext4Error::NotFound) => {
                    return Self::linear_lookup(reader, super_block_manager, dir_inode, name);
                }
                Err(_) => return Self::linear_lookup(reader, super_block_manager, dir_inode, name),
            }
        }
        Self::linear_lookup(reader, super_block_manager, dir_inode, name)
    }

    /// Linear (brute-force) lookup in a directory.
    pub fn linear_lookup<D: BlockDevice>(
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

    /// HTree-accelerated lookup with multi-level support and collision handling.
    pub fn htree_lookup<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &Inode,
        name: &str,
    ) -> Result<u32> {
        let bs = super_block_manager.block_size;
        let has_filetype =
            (super_block_manager.super_block.s_feature_incompat & INCOMPAT_FILETYPE) != 0;

        // 1. Read logical block 0 as htree root.
        let root_map =
            ExtentWalker::logical_to_physical(reader, super_block_manager, dir_inode, 0)?;
        let root_physical = root_map
            .ok_or(Ext4Error::CorruptedFs("htree root block not mapped"))?
            .physical_block;
        let mut block = vec![0u8; bs];
        reader.read_block(root_physical, &mut block)?;
        let dx = DxRoot::parse(&block)?;

        // 2. Compute ext4-compatible hash using s_hash_seed.
        let hash = compute_hash(
            name.as_bytes(),
            dx.hash_version,
            &super_block_manager.super_block.s_hash_seed,
        );

        // 3. Descend through intermediate levels (if any).
        let mut current_entries = dx.entries.clone();
        let mut levels = dx.indirection_levels;

        while levels > 0 {
            let target_logical = lookup_in_entries(&current_entries, hash);
            let phys = Self::resolve_logical_block(
                reader,
                super_block_manager,
                dir_inode,
                target_logical,
            )?;
            reader.read_block(phys, &mut block)?;
            let node = DxNode::parse(&block)?;
            current_entries = node.entries;
            levels -= 1;
        }

        // 4. current_entries now point to leaf directory blocks.
        //    Collect all candidate blocks (handles hash collisions).
        let candidates = find_candidate_blocks(&current_entries, hash);

        // 5. Scan each candidate block for the name.
        for logical_block in candidates {
            let phys =
                Self::resolve_logical_block(reader, super_block_manager, dir_inode, logical_block)?;
            reader.read_block(phys, &mut block)?;
            if let Some(ino) = Self::scan_block_for_name(&block, has_filetype, name)? {
                return Ok(ino);
            }
        }

        Err(Ext4Error::NotFound)
    }

    /// Resolve a logical block number to a physical block via extent tree.
    fn resolve_logical_block<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        dir_inode: &Inode,
        logical_block: u32,
    ) -> Result<u64> {
        let map = ExtentWalker::logical_to_physical(
            reader,
            super_block_manager,
            dir_inode,
            logical_block,
        )?;
        Ok(map
            .ok_or(Ext4Error::CorruptedFs("htree block not mapped"))?
            .physical_block)
    }

    /// Scan a single directory data block for a name.
    fn scan_block_for_name(raw: &[u8], has_filetype: bool, name: &str) -> Result<Option<u32>> {
        let mut off = 0usize;
        while off < raw.len() {
            let entry = DirEntry::parse(&raw[off..], has_filetype)?;
            let rec_len = entry.rec_len as usize;
            if rec_len == 0 {
                return Err(Ext4Error::CorruptedFs("dir entry rec_len is zero"));
            }
            if !entry.is_unused() && entry.name == name {
                return Ok(Some(entry.inode));
            }
            off += rec_len;
        }
        Ok(None)
    }
}

/// Find the target block in a sorted dx entry list (same logic as DxRoot/DxNode::lookup_block).
fn lookup_in_entries(entries: &[crate::layout::htree::DxEntry], hash: u32) -> u32 {
    let mut chosen = entries[0].block;
    for e in &entries[1..] {
        if e.hash <= hash {
            chosen = e.block;
        } else {
            break;
        }
    }
    chosen
}
