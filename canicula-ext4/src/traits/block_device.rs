use crate::error::Ext4Error;

/// Block device abstraction — the I/O foundation for the entire filesystem.
///
/// Implementations may back onto real disk, memory image, or network block device.
pub trait BlockDevice {
    /// Read a single block into `buf`.
    ///
    /// `buf.len()` must equal `self.block_size()`.
    fn read_block(&self, block_no: u64, buf: &mut [u8]) -> ::core::result::Result<(), Ext4Error>;

    /// Write `buf` to a single block.
    ///
    /// Read-only implementations may return `Err(ReadOnly)`.
    fn write_block(&mut self, block_no: u64, buf: &[u8]) -> ::core::result::Result<(), Ext4Error>;

    /// Block size in bytes (typically 1024 / 2048 / 4096).
    fn block_size(&self) -> usize;

    /// Total number of blocks on the device.
    fn total_blocks(&self) -> u64;

    /// Flush all pending writes to stable storage.
    fn flush(&mut self) -> ::core::result::Result<(), Ext4Error>;
}

impl<T: BlockDevice + ?Sized> BlockDevice for &mut T {
    fn read_block(&self, block_no: u64, buf: &mut [u8]) -> ::core::result::Result<(), Ext4Error> {
        (**self).read_block(block_no, buf)
    }

    fn write_block(&mut self, block_no: u64, buf: &[u8]) -> ::core::result::Result<(), Ext4Error> {
        (**self).write_block(block_no, buf)
    }

    fn block_size(&self) -> usize {
        (**self).block_size()
    }

    fn total_blocks(&self) -> u64 {
        (**self).total_blocks()
    }

    fn flush(&mut self) -> ::core::result::Result<(), Ext4Error> {
        (**self).flush()
    }
}

impl<T: BlockDevice + ?Sized> BlockDevice for &T {
    fn read_block(&self, block_no: u64, buf: &mut [u8]) -> ::core::result::Result<(), Ext4Error> {
        (**self).read_block(block_no, buf)
    }

    fn write_block(
        &mut self,
        _block_no: u64,
        _buf: &[u8],
    ) -> ::core::result::Result<(), Ext4Error> {
        Err(Ext4Error::ReadOnly)
    }

    fn block_size(&self) -> usize {
        (**self).block_size()
    }

    fn total_blocks(&self) -> u64 {
        (**self).total_blocks()
    }

    fn flush(&mut self) -> ::core::result::Result<(), Ext4Error> {
        Ok(())
    }
}
