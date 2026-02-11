use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::block_group_manager::BlockGroupManager;
use crate::fs_core::dir_reader::DirReader;
use crate::fs_core::inode_reader::InodeReader;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::fs_core::symlink::SymlinkReader;
use crate::io::block_reader::BlockReader;
use crate::traits::block_device::BlockDevice;

/// Maximum symlink expansion depth.
pub const MAX_SYMLINK_DEPTH: u32 = 40;

/// Path resolver: convert absolute paths into inode numbers.
pub struct PathResolver;

impl PathResolver {
    /// Resolve absolute path to inode number.
    pub fn resolve<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        block_group_manager: &BlockGroupManager,
        path: &str,
    ) -> Result<u32> {
        if !path.starts_with('/') {
            return Err(Ext4Error::CorruptedFs("path must be absolute"));
        }

        let mut pending = Self::split_components(path);
        let mut current_ino = 2u32; // root inode in ext4
        let mut symlink_depth = 0u32;

        while let Some(component) = pending.pop_front() {
            if component == "." {
                continue;
            }

            let current_inode = InodeReader::read_inode(
                reader,
                super_block_manager,
                block_group_manager,
                current_ino,
            )?;
            if !current_inode.is_dir() {
                return Err(Ext4Error::NotDirectory);
            }

            let next_ino =
                DirReader::lookup(reader, super_block_manager, &current_inode, &component)?;
            let next_inode = InodeReader::read_inode(
                reader,
                super_block_manager,
                block_group_manager,
                next_ino,
            )?;

            if next_inode.is_symlink() {
                symlink_depth += 1;
                if symlink_depth > MAX_SYMLINK_DEPTH {
                    return Err(Ext4Error::SymlinkLoop(symlink_depth));
                }

                let target = SymlinkReader::read_symlink(reader, super_block_manager, &next_inode)?;
                let mut new_pending = Self::split_components(&target);

                if target.starts_with('/') {
                    current_ino = 2;
                }

                // prepend symlink target components before remaining components
                while let Some(rem) = pending.pop_front() {
                    new_pending.push_back(rem);
                }
                pending = new_pending;
            } else {
                current_ino = next_ino;
            }
        }

        Ok(current_ino)
    }

    /// Resolve parent directory and final name for absolute path.
    pub fn resolve_parent<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        block_group_manager: &BlockGroupManager,
        path: &str,
    ) -> Result<(u32, String)> {
        if !path.starts_with('/') {
            return Err(Ext4Error::CorruptedFs("path must be absolute"));
        }

        let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
        if components.is_empty() {
            return Err(Ext4Error::CorruptedFs("root has no parent"));
        }

        let name = components[components.len() - 1].to_string();
        if components.len() == 1 {
            return Ok((2, name));
        }

        let parent_path = format!("/{}", components[..components.len() - 1].join("/"));
        let parent_ino = Self::resolve(
            reader,
            super_block_manager,
            block_group_manager,
            &parent_path,
        )?;
        Ok((parent_ino, name))
    }

    fn split_components(path: &str) -> VecDeque<String> {
        path.split('/')
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string())
            .collect()
    }
}
