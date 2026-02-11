use alloc::string::String;
use alloc::vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::file_reader::FileReader;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::inode::Inode;
use crate::traits::block_device::BlockDevice;

/// Symlink reader.
pub struct SymlinkReader;

impl SymlinkReader {
    /// Read symlink target path.
    ///
    /// Fast symlink (`i_blocks == 0 && i_size <= 60`) is stored directly in `i_block`.
    /// Otherwise read through normal file data path.
    pub fn read_symlink<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
    ) -> Result<String> {
        if !inode.is_symlink() {
            return Err(Ext4Error::CorruptedFs("inode is not a symlink"));
        }

        let len = inode.i_size as usize;
        if inode.i_blocks == 0 && len <= inode.i_block.len() {
            let bytes = &inode.i_block[..len];
            let s = core::str::from_utf8(bytes)
                .map_err(|_| Ext4Error::CorruptedFs("symlink target not utf8"))?;
            return Ok(s.into());
        }

        let mut buf = vec![0u8; len];
        let n = FileReader::read(reader, super_block_manager, inode, 0, &mut buf)?;
        let s = core::str::from_utf8(&buf[..n])
            .map_err(|_| Ext4Error::CorruptedFs("symlink target not utf8"))?;
        Ok(s.into())
    }
}
