#![allow(dead_code)]

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_alloc::bitmap::{clear_bit, find_first_zero, set_bit, test_bit};
use crate::traits::allocator::InodeAllocator;

#[derive(Clone, Debug)]
pub struct InodeGroupAllocState {
    pub inode_bitmap: Vec<u8>,
    pub free_inodes_count: u32,
    pub free_blocks_count: u32,
    pub used_dirs_count: u32,
    pub max_bits: usize,
}

/// In-memory ext4 inode allocator model.
///
/// Tracks dirty groups for bitmap and descriptor writeback.
pub struct Ext4InodeAllocator {
    pub inodes_per_group: u32,
    pub free_inodes_total: u64,
    groups: Vec<InodeGroupAllocState>,
    /// Inode groups whose bitmaps have been modified since last flush.
    dirty_groups: BTreeSet<usize>,
}

impl Ext4InodeAllocator {
    pub fn new(inodes_per_group: u32, groups: Vec<InodeGroupAllocState>) -> Self {
        let free_inodes_total = groups.iter().map(|g| g.free_inodes_count as u64).sum();
        Self {
            inodes_per_group,
            free_inodes_total,
            groups,
            dirty_groups: BTreeSet::new(),
        }
    }

    /// Return and clear the set of groups that were modified since the last drain.
    pub fn drain_dirty_groups(&mut self) -> BTreeSet<usize> {
        core::mem::take(&mut self.dirty_groups)
    }

    /// Get the bitmap bytes for the given group (for writeback).
    pub fn group_bitmap(&self, group_no: usize) -> &[u8] {
        &self.groups[group_no].inode_bitmap
    }

    /// Get the current free inode count for the given group.
    pub fn group_free_count(&self, group_no: usize) -> u32 {
        self.groups[group_no].free_inodes_count
    }

    /// Get the current used dirs count for the given group.
    pub fn group_used_dirs(&self, group_no: usize) -> u32 {
        self.groups[group_no].used_dirs_count
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    fn group_for_inode(&self, ino: u32) -> Result<usize> {
        if ino == 0 {
            return Err(Ext4Error::CorruptedFs("inode number starts from 1"));
        }
        let idx = (ino - 1) / self.inodes_per_group;
        let group = idx as usize;
        if group >= self.groups.len() {
            return Err(Ext4Error::CorruptedFs(
                "inode number out of allocator range",
            ));
        }
        Ok(group)
    }

    fn scan_from(&self, start_group: usize) -> Option<usize> {
        if self.groups.is_empty() {
            return None;
        }
        for step in 0..self.groups.len() {
            let g = (start_group + step) % self.groups.len();
            if self.groups[g].free_inodes_count > 0 {
                return Some(g);
            }
        }
        None
    }

    fn choose_group_orlov(&self, parent_group: usize) -> Option<usize> {
        if self.groups.is_empty() {
            return None;
        }

        let n = self.groups.len() as u64;
        let avg_free_inodes = self
            .groups
            .iter()
            .map(|g| g.free_inodes_count as u64)
            .sum::<u64>()
            / n;
        let avg_free_blocks = self
            .groups
            .iter()
            .map(|g| g.free_blocks_count as u64)
            .sum::<u64>()
            / n;
        let avg_used_dirs = self
            .groups
            .iter()
            .map(|g| g.used_dirs_count as u64)
            .sum::<u64>()
            / n;

        for step in 0..self.groups.len() {
            let g = (parent_group + step) % self.groups.len();
            let st = &self.groups[g];
            if st.free_inodes_count as u64 > avg_free_inodes
                && st.free_blocks_count as u64 > avg_free_blocks
                && st.used_dirs_count as u64 <= avg_used_dirs
            {
                return Some(g);
            }
        }

        self.scan_from(parent_group)
    }
}

impl InodeAllocator for Ext4InodeAllocator {
    fn alloc_inode(&mut self, parent_inode: u32, is_dir: bool) -> Result<u32> {
        if self.free_inodes_total == 0 || self.groups.is_empty() {
            return Err(Ext4Error::NoSpace);
        }

        let parent_group = self.group_for_inode(parent_inode).unwrap_or(0);
        let selected = if is_dir {
            self.choose_group_orlov(parent_group)
        } else {
            self.scan_from(parent_group)
        }
        .ok_or(Ext4Error::NoSpace)?;

        let g = &mut self.groups[selected];
        let bit = find_first_zero(&g.inode_bitmap, 0, g.max_bits).ok_or(Ext4Error::CorruptedFs(
            "group free inode count inconsistent with bitmap",
        ))?;

        set_bit(&mut g.inode_bitmap, bit);
        g.free_inodes_count -= 1;
        if is_dir {
            g.used_dirs_count += 1;
        }
        self.free_inodes_total -= 1;
        self.dirty_groups.insert(selected);

        let ino = selected as u32 * self.inodes_per_group + bit as u32 + 1;
        Ok(ino)
    }

    fn free_inode(&mut self, ino: u32) -> Result<()> {
        let group_no = self.group_for_inode(ino)?;
        let bit = ((ino - 1) % self.inodes_per_group) as usize;
        let g = &mut self.groups[group_no];
        if bit >= g.max_bits {
            return Err(Ext4Error::CorruptedFs("inode bit out of group range"));
        }
        if !test_bit(&g.inode_bitmap, bit) {
            return Err(Ext4Error::CorruptedFs("double free inode"));
        }

        clear_bit(&mut g.inode_bitmap, bit);
        g.free_inodes_count += 1;
        self.free_inodes_total += 1;
        self.dirty_groups.insert(group_no);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{Ext4InodeAllocator, InodeGroupAllocState};
    use crate::traits::allocator::InodeAllocator;

    #[test]
    fn test_alloc_inode_prefers_parent_group_for_files() {
        let groups = vec![
            InodeGroupAllocState {
                inode_bitmap: vec![0b0000_0001],
                free_inodes_count: 7,
                free_blocks_count: 100,
                used_dirs_count: 10,
                max_bits: 8,
            },
            InodeGroupAllocState {
                inode_bitmap: vec![0b0000_0000],
                free_inodes_count: 8,
                free_blocks_count: 100,
                used_dirs_count: 0,
                max_bits: 8,
            },
        ];
        let mut alloc = Ext4InodeAllocator::new(8, groups);

        // parent ino=2 => group 0
        let ino = alloc.alloc_inode(2, false).unwrap();
        assert_eq!(ino, 2);
    }

    #[test]
    fn test_alloc_inode_orlov_spreads_directories() {
        let groups = vec![
            InodeGroupAllocState {
                inode_bitmap: vec![0b0000_1111],
                free_inodes_count: 4,
                free_blocks_count: 8,
                used_dirs_count: 8,
                max_bits: 8,
            },
            InodeGroupAllocState {
                inode_bitmap: vec![0b0000_0000],
                free_inodes_count: 8,
                free_blocks_count: 16,
                used_dirs_count: 1,
                max_bits: 8,
            },
        ];
        let mut alloc = Ext4InodeAllocator::new(8, groups);

        let ino = alloc.alloc_inode(2, true).unwrap();
        assert!(ino >= 9, "expected Orlov to choose group 1");
    }

    #[test]
    fn test_free_inode_restores_free_count() {
        let groups = vec![InodeGroupAllocState {
            inode_bitmap: vec![0u8],
            free_inodes_count: 8,
            free_blocks_count: 32,
            used_dirs_count: 0,
            max_bits: 8,
        }];
        let mut alloc = Ext4InodeAllocator::new(8, groups);

        let ino = alloc.alloc_inode(2, false).unwrap();
        assert_eq!(alloc.free_inodes_total, 7);
        alloc.free_inode(ino).unwrap();
        assert_eq!(alloc.free_inodes_total, 8);
    }
}
