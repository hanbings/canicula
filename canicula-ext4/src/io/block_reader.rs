use crate::error::{Ext4Error, Result};
use crate::traits::block_device::BlockDevice;

/// Block reader wrapping a [`BlockDevice`] with higher-level read operations.
///
/// Thin convenience layer that translates byte offsets and multi-block reads
/// into individual `BlockDevice::read_block` calls.
pub struct BlockReader<D: BlockDevice> {
    device: D,
}

impl<D: BlockDevice> BlockReader<D> {
    /// Create a new reader wrapping the given block device.
    pub fn new(device: D) -> Self {
        Self { device }
    }

    /// Read a single block into `buf`.
    ///
    /// `buf.len()` must equal `self.block_size()`.
    pub fn read_block(&self, block_no: u64, buf: &mut [u8]) -> Result<()> {
        self.device.read_block(block_no, buf)
    }

    /// Read `buf.len()` bytes starting at the given **byte** offset.
    ///
    /// Handles cross-block boundaries transparently.
    /// Used for reading the super block (at byte offset 1024), etc.
    ///
    /// Note: uses a 4096-byte stack buffer; block sizes > 4096 are not
    /// supported by this method (returns `Err(IoError)`).
    pub fn read_bytes(&self, byte_offset: u64, buf: &mut [u8]) -> Result<()> {
        let bs = self.device.block_size();
        if bs > 4096 {
            return Err(Ext4Error::IoError);
        }

        let mut current_block = byte_offset / bs as u64;
        let mut offset_in_block = (byte_offset % bs as u64) as usize;
        let mut written = 0usize;

        // Stack-allocated scratch buffer (covers block sizes up to 4096).
        let mut block_buf = [0u8; 4096];

        while written < buf.len() {
            self.device
                .read_block(current_block, &mut block_buf[..bs])?;

            let available = bs - offset_in_block;
            let to_copy = if available < buf.len() - written {
                available
            } else {
                buf.len() - written
            };

            buf[written..written + to_copy]
                .copy_from_slice(&block_buf[offset_in_block..offset_in_block + to_copy]);

            written += to_copy;
            current_block += 1;
            offset_in_block = 0; // subsequent blocks start at byte 0
        }

        Ok(())
    }

    /// Read `count` consecutive blocks starting at `start_block` into `buf`.
    ///
    /// `buf.len()` must equal `count * self.block_size()`.
    pub fn read_blocks(&self, start_block: u64, count: u64, buf: &mut [u8]) -> Result<()> {
        let bs = self.device.block_size();
        for i in 0..count {
            let offset = (i as usize) * bs;
            self.device
                .read_block(start_block + i, &mut buf[offset..offset + bs])?;
        }
        Ok(())
    }

    /// Block size reported by the underlying device.
    pub fn block_size(&self) -> usize {
        self.device.block_size()
    }

    /// Borrow the underlying device.
    pub fn device(&self) -> &D {
        &self.device
    }
}
