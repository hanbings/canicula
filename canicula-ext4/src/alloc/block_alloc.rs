#![allow(dead_code)]

use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_alloc::bitmap::{clear_bit, find_first_zero, set_bit, test_bit};
use crate::traits::allocator::BlockAllocator;

#[derive(Clone, Debug)]
pub struct BlockGroupAllocState {
    pub block_bitmap: Vec<u8>,
    pub free_blocks_count: u32,
    pub max_bits: usize,
}

/// In-memory ext4 block allocator model for Phase 7.
///
/// This provides policy + bitmap operations and can later be wired to
/// on-disk bitmap/descriptor writeback paths.
pub struct Ext4BlockAllocator {
    pub first_data_block: u64,
    pub blocks_per_group: u32,
    pub free_blocks_total: u64,
    groups: Vec<BlockGroupAllocState>,
}

impl Ext4BlockAllocator {
    pub fn new(
        first_data_block: u64,
        blocks_per_group: u32,
        groups: Vec<BlockGroupAllocState>,
    ) -> Self {
        let free_blocks_total = groups.iter().map(|g| g.free_blocks_count as u64).sum();
        Self {
            first_data_block,
            blocks_per_group,
            free_blocks_total,
            groups,
        }
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn group_free_blocks(&self, group_no: usize) -> u32 {
        self.groups[group_no].free_blocks_count
    }

    fn goal_group(&self, goal: u64) -> usize {
        if self.groups.is_empty() {
            return 0;
        }
        if goal <= self.first_data_block {
            return 0;
        }
        let rel = goal - self.first_data_block;
        let group = (rel / self.blocks_per_group as u64) as usize;
        group % self.groups.len()
    }

    fn alloc_in_group(
        &mut self,
        group_no: usize,
        start_bit: usize,
        remaining: usize,
        out_blocks: &mut Vec<u64>,
    ) {
        let g = &mut self.groups[group_no];
        let mut next_start = start_bit.min(g.max_bits);

        while out_blocks.len() < remaining {
            let bit = match find_first_zero(&g.block_bitmap, next_start, g.max_bits) {
                Some(bit) => bit,
                None => break,
            };
            set_bit(&mut g.block_bitmap, bit);
            g.free_blocks_count -= 1;
            self.free_blocks_total -= 1;
            let pblk = self.first_data_block
                + (group_no as u64 * self.blocks_per_group as u64)
                + bit as u64;
            out_blocks.push(pblk);
            next_start = bit + 1;
        }
    }
}

impl BlockAllocator for Ext4BlockAllocator {
    fn alloc_blocks(&mut self, goal: u64, count: usize) -> Result<Vec<u64>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        if self.free_blocks_total < count as u64 {
            return Err(Ext4Error::NoSpace);
        }
        if self.groups.is_empty() {
            return Err(Ext4Error::NoSpace);
        }

        let goal_group = self.goal_group(goal);
        let mut allocated = Vec::with_capacity(count);

        for step in 0..self.groups.len() {
            if allocated.len() == count {
                break;
            }
            let group_no = (goal_group + step) % self.groups.len();
            if self.groups[group_no].free_blocks_count == 0 {
                continue;
            }

            let start_bit = if step == 0 && goal > self.first_data_block {
                ((goal - self.first_data_block) % self.blocks_per_group as u64) as usize
            } else {
                0
            };
            self.alloc_in_group(group_no, start_bit, count, &mut allocated);
        }

        if allocated.len() != count {
            // Roll back on partial allocation.
            self.free_blocks(&allocated)?;
            return Err(Ext4Error::NoSpace);
        }
        Ok(allocated)
    }

    fn free_blocks(&mut self, blocks: &[u64]) -> Result<()> {
        for &pblk in blocks {
            if pblk < self.first_data_block {
                return Err(Ext4Error::CorruptedFs("free block below first_data_block"));
            }
            let rel = pblk - self.first_data_block;
            let group_no = (rel / self.blocks_per_group as u64) as usize;
            if group_no >= self.groups.len() {
                return Err(Ext4Error::CorruptedFs("free block out of allocator range"));
            }
            let bit = (rel % self.blocks_per_group as u64) as usize;

            let g = &mut self.groups[group_no];
            if bit >= g.max_bits {
                return Err(Ext4Error::CorruptedFs("free block bit out of group range"));
            }
            if !test_bit(&g.block_bitmap, bit) {
                return Err(Ext4Error::CorruptedFs("double free block"));
            }

            clear_bit(&mut g.block_bitmap, bit);
            g.free_blocks_count += 1;
            self.free_blocks_total += 1;
        }
        Ok(())
    }

    fn free_block_count(&self) -> u64 {
        self.free_blocks_total
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{BlockGroupAllocState, Ext4BlockAllocator};
    use crate::traits::allocator::BlockAllocator;

    #[test]
    fn test_alloc_blocks_goal_group_then_round_robin() {
        let groups = vec![
            BlockGroupAllocState {
                block_bitmap: vec![0b1111_1111],
                free_blocks_count: 0,
                max_bits: 8,
            },
            BlockGroupAllocState {
                block_bitmap: vec![0b0000_1111],
                free_blocks_count: 4,
                max_bits: 8,
            },
        ];
        let mut alloc = Ext4BlockAllocator::new(0, 8, groups);

        let blocks = alloc.alloc_blocks(1, 2).unwrap();
        assert_eq!(blocks, vec![12, 13]);
        assert_eq!(alloc.group_free_blocks(1), 2);
        assert_eq!(alloc.free_block_count(), 2);
    }

    #[test]
    fn test_free_blocks_restores_counters() {
        let groups = vec![BlockGroupAllocState {
            block_bitmap: vec![0u8],
            free_blocks_count: 8,
            max_bits: 8,
        }];
        let mut alloc = Ext4BlockAllocator::new(100, 8, groups);

        let blocks = alloc.alloc_blocks(100, 3).unwrap();
        assert_eq!(alloc.free_block_count(), 5);
        alloc.free_blocks(&blocks).unwrap();
        assert_eq!(alloc.free_block_count(), 8);
    }
}
