use alloc::vec;

use crate::error::Result;
use crate::fs_core::extent_walker::ExtentWalker;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_reader::BlockReader;
use crate::layout::inode::Inode;
use crate::traits::block_device::BlockDevice;

/// File data reader.
///
/// Reads bytes from a file inode by resolving logical blocks through the extent tree.
pub struct FileReader;

impl FileReader {
    /// Read file bytes at `offset` into `buf`.
    ///
    /// Returns the number of bytes actually read (EOF-aware).
    pub fn read<D: BlockDevice>(
        reader: &BlockReader<D>,
        super_block_manager: &SuperBlockManager,
        inode: &Inode,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize> {
        if buf.is_empty() || offset >= inode.i_size {
            return Ok(0);
        }

        let block_size = super_block_manager.block_size;
        let file_remaining = (inode.i_size - offset) as usize;
        let to_read = core::cmp::min(buf.len(), file_remaining);

        let mut scratch = vec![0u8; block_size];
        let mut copied = 0usize;
        let mut current_logical = (offset / block_size as u64) as u32;
        let mut offset_in_block = (offset % block_size as u64) as usize;

        while copied < to_read {
            let in_this_block = core::cmp::min(block_size - offset_in_block, to_read - copied);
            let mapping = ExtentWalker::logical_to_physical(
                reader,
                super_block_manager,
                inode,
                current_logical,
            )?;

            match mapping {
                Some(m) if !m.uninitialized => {
                    reader.read_block(m.physical_block, &mut scratch)?;
                    buf[copied..copied + in_this_block].copy_from_slice(
                        &scratch[offset_in_block..offset_in_block + in_this_block],
                    );
                }
                _ => {
                    // Sparse hole or uninitialized extent reads as zeros.
                    buf[copied..copied + in_this_block].fill(0);
                }
            }

            copied += in_this_block;
            current_logical += 1;
            offset_in_block = 0;
        }

        Ok(copied)
    }
}
