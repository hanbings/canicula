use alloc::vec::Vec;

use crate::error::Result;

/// Allocates and frees physical blocks for write paths.
pub trait BlockAllocator {
    /// Allocate up to `count` blocks with locality hint `goal`.
    ///
    /// Implementations may ignore the hint when needed.
    fn alloc_blocks(&mut self, goal: u64, count: usize) -> Result<Vec<u64>>;

    /// Release previously allocated physical blocks.
    fn free_blocks(&mut self, blocks: &[u64]) -> Result<()>;

    /// Total remaining free block count.
    fn free_block_count(&self) -> u64;
}

/// Allocates and frees inode numbers.
pub trait InodeAllocator {
    /// Allocate one inode under `parent_inode`.
    ///
    /// `is_dir` allows policy differences (e.g. Orlov for directories).
    fn alloc_inode(&mut self, parent_inode: u32, is_dir: bool) -> Result<u32>;

    /// Release one inode number.
    fn free_inode(&mut self, ino: u32) -> Result<()>;
}
