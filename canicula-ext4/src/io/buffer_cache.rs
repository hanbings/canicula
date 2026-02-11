use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::Result;
use crate::io::block_reader::BlockReader;
use crate::traits::block_device::BlockDevice;

#[derive(Debug, Clone)]
struct CachedBlock {
    block_no: u64,
    data: Vec<u8>,
    pin: bool,
}

/// Simple block buffer cache with LRU-like eviction.
///
/// Phase 3 only needs read caching, so this cache focuses on `get_block()`.
pub struct BufferCache<D: BlockDevice> {
    reader: BlockReader<D>,
    capacity: usize,
    entries: VecDeque<CachedBlock>,
}

impl<D: BlockDevice> BufferCache<D> {
    /// Create a cache with at most `capacity` cached blocks.
    pub fn new(reader: BlockReader<D>, capacity: usize) -> Self {
        Self {
            reader,
            capacity,
            entries: VecDeque::new(),
        }
    }

    /// Read a block through cache and return an immutable view.
    pub fn get_block(&mut self, block_no: u64) -> Result<&[u8]> {
        if let Some(idx) = self.entries.iter().position(|e| e.block_no == block_no) {
            // Move hit to the back so older entries are evicted first.
            let hit = self.entries.remove(idx).expect("index valid");
            self.entries.push_back(hit);
            let last = self.entries.back().expect("just pushed");
            return Ok(&last.data);
        }

        let bs = self.reader.block_size();
        let mut data = vec![0u8; bs];
        self.reader.read_block(block_no, &mut data)?;
        self.entries.push_back(CachedBlock {
            block_no,
            data,
            pin: false,
        });
        self.evict_if_needed();

        let last = self.entries.back().expect("just pushed");
        Ok(&last.data)
    }

    /// Drop one cached block.
    pub fn invalidate(&mut self, block_no: u64) {
        if let Some(idx) = self.entries.iter().position(|e| e.block_no == block_no) {
            self.entries.remove(idx);
        }
    }

    /// Drop all cached blocks.
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    /// Pin a block so eviction skips it.
    pub fn pin(&mut self, block_no: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.block_no == block_no) {
            entry.pin = true;
        }
    }

    /// Unpin a previously pinned block.
    pub fn unpin(&mut self, block_no: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.block_no == block_no) {
            entry.pin = false;
        }
    }

    /// Number of blocks currently cached.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no blocks are currently cached.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn evict_if_needed(&mut self) {
        if self.capacity == 0 {
            self.entries.clear();
            return;
        }
        while self.entries.len() > self.capacity {
            // Evict the first non-pinned entry from the front.
            if let Some(idx) = self.entries.iter().position(|e| !e.pin) {
                self.entries.remove(idx);
            } else {
                // All blocks are pinned.
                break;
            }
        }
    }
}
