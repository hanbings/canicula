use crate::error::{Ext4Error, Result};
use crate::fs_core::block_group_manager::BlockGroupManager;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::inode::Inode;
use crate::traits::block_device::BlockDevice;

/// Maximum supported inode size (stack buffer limit).
const MAX_INODE_SIZE: usize = 1024;

/// Inode reader — locates and reads inodes from disk.
///
/// Stateless: all context is passed as parameters.
pub struct InodeReader;

impl InodeReader {
    /// Read and parse the inode with the given inode number.
    ///
    /// 1. Validate `ino` > 0 and <= `s_inodes_count` (inode 0 does not exist).
    /// 2. `group = (ino - 1) / s_inodes_per_group`
    /// 3. `index = (ino - 1) % s_inodes_per_group`
    /// 4. `table_block = block_group_manager.inode_table_block(group)`
    /// 5. `byte_offset = table_block * block_size + index * inode_size`
    /// 6. Read `inode_size` bytes → `Inode::parse()`
    pub fn read_inode<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        block_group_manager: &BlockGroupManager,
        ino: u32,
    ) -> Result<Inode> {
        let super_block = &super_block_manager.super_block;

        // Validate inode number (inode 0 does not exist; numbering starts at 1)
        if ino == 0 || ino > super_block.s_inodes_count {
            return Err(Ext4Error::CorruptedFs("inode number out of range"));
        }

        let inodes_per_group = super_block.s_inodes_per_group;
        let inode_size = super_block.s_inode_size as usize;
        let block_size = super_block_manager.block_size;

        if inode_size > MAX_INODE_SIZE {
            return Err(Ext4Error::CorruptedFs("inode_size exceeds supported limit"));
        }

        // Locate the inode
        let group = (ino - 1) / inodes_per_group;
        let index = (ino - 1) % inodes_per_group;
        let table_block = block_group_manager.inode_table_block(group);
        let byte_offset = table_block * block_size as u64 + index as u64 * inode_size as u64;

        // Read raw inode bytes
        let mut inode_buf = [0u8; MAX_INODE_SIZE];
        reader.read_bytes(byte_offset, &mut inode_buf[..inode_size])?;

        // Parse
        Inode::parse(&inode_buf[..inode_size], super_block.s_inode_size)
    }

    /// Read the root directory inode (always inode 2 in ext4).
    pub fn read_root_inode<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        block_group_manager: &BlockGroupManager,
    ) -> Result<Inode> {
        Self::read_inode(reader, super_block_manager, block_group_manager, 2)
    }
}
