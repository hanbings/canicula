use alloc::vec;

use crate::error::{Ext4Error, Result};
use crate::io::block_reader::BlockReader;
use crate::traits::block_device::BlockDevice;

/// Block writer wrapping a [`BlockDevice`] with higher-level write operations.
pub struct BlockWriter<D: BlockDevice> {
    device: D,
}

impl<D: BlockDevice> BlockWriter<D> {
    /// Create a new writer wrapping the given block device.
    pub fn new(device: D) -> Self {
        Self { device }
    }

    /// Write a single block from `data`.
    ///
    /// `data.len()` must equal block size.
    pub fn write_block(&mut self, block_no: u64, data: &[u8]) -> Result<()> {
        if data.len() != self.device.block_size() {
            return Err(Ext4Error::IoError);
        }
        self.device.write_block(block_no, data)
    }

    /// Write bytes at arbitrary byte offset.
    ///
    /// Uses read-modify-write for partial block writes.
    pub fn write_bytes(&mut self, byte_offset: u64, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let bs = self.device.block_size();
        if bs > 4096 {
            return Err(Ext4Error::IoError);
        }

        let mut current_block = byte_offset / bs as u64;
        let mut offset_in_block = (byte_offset % bs as u64) as usize;
        let mut consumed = 0usize;
        let mut block_buf = [0u8; 4096];

        while consumed < data.len() {
            let to_copy = core::cmp::min(bs - offset_in_block, data.len() - consumed);
            let whole_block = to_copy == bs && offset_in_block == 0;

            if !whole_block {
                self.device
                    .read_block(current_block, &mut block_buf[..bs])?;
            }

            block_buf[offset_in_block..offset_in_block + to_copy]
                .copy_from_slice(&data[consumed..consumed + to_copy]);
            self.device.write_block(current_block, &block_buf[..bs])?;

            consumed += to_copy;
            current_block += 1;
            offset_in_block = 0;
        }

        Ok(())
    }

    /// Zero a range of consecutive blocks.
    pub fn zero_blocks(&mut self, start_block: u64, count: u64) -> Result<()> {
        let bs = self.device.block_size();
        let zeros = vec![0u8; bs];
        for i in 0..count {
            self.device.write_block(start_block + i, &zeros)?;
        }
        Ok(())
    }

    /// Flush pending writes.
    pub fn flush(&mut self) -> Result<()> {
        self.device.flush()
    }

    /// Block size reported by underlying device.
    pub fn block_size(&self) -> usize {
        self.device.block_size()
    }

    /// Borrow the underlying device.
    pub fn device(&self) -> &D {
        &self.device
    }

    /// Mutably borrow the underlying device.
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    /// Build a temporary read wrapper borrowing the same device.
    pub fn as_reader(&self) -> BlockReader<&D> {
        BlockReader::new(&self.device)
    }
}
