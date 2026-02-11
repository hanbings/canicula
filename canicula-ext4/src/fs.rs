use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::fs_alloc::block_alloc::BlockGroupAllocState;
use crate::fs_alloc::block_alloc::Ext4BlockAllocator;
use crate::fs_alloc::inode_alloc::Ext4InodeAllocator;
use crate::fs_alloc::inode_alloc::InodeGroupAllocState;
use crate::fs_core::block_group_manager::BlockGroupManager;
use crate::fs_core::dir_reader::DirReader;
use crate::fs_core::dir_writer::DirWriter;
use crate::fs_core::extent_modifier::ExtentModifier;
use crate::fs_core::file_reader::FileReader;
use crate::fs_core::file_writer::FileWriter;
use crate::fs_core::inode_reader::InodeReader;
use crate::fs_core::inode_writer::InodeWriter;
use crate::fs_core::path_resolver::PathResolver;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::fs_core::symlink::SymlinkReader;
use crate::io::block_reader::BlockReader;
use crate::io::block_writer::BlockWriter;
use crate::journal::engine::Jbd2Journal;
use crate::journal::jbd2_superblock::{
    JBD2_BLOCKTYPE_SUPERBLOCK_V2, JBD2_MAGIC_NUMBER, JournalHeader, JournalSuperBlock,
};
use crate::layout::checksum::block_group_checksum;
use crate::layout::dir_entry::FileType as DirEntryFileType;
use crate::layout::inode::{FileType as InodeFileType, Inode, S_IFDIR, S_IFLNK};
use crate::traits::allocator::{BlockAllocator, InodeAllocator};
use crate::traits::block_device::BlockDevice;
use crate::traits::journal::Journal;
use crate::traits::vfs::{FileSystem, InodeOps, StatFs};

/// Main ext4 filesystem object that wires all modules together.
pub struct Ext4FileSystem<D: BlockDevice> {
    pub device: D,
    pub sb_manager: SuperBlockManager,
    pub bg_manager: BlockGroupManager,
    pub read_only: bool,
    pub block_allocator: Option<Ext4BlockAllocator>,
    pub inode_allocator: Option<Ext4InodeAllocator>,
    pub journal: Option<FsJournalState>,
    /// Tracks metadata blocks dirtied since last journal commit.
    dirty_blocks: BTreeSet<u64>,
}

#[derive(Clone)]
pub struct FsJournalState {
    pub start_block: u64,
    pub superblock: JournalSuperBlock,
    pub has_64bit: bool,
    pub has_csum: bool,
}

impl<D: BlockDevice> Ext4FileSystem<D> {
    pub fn mount(device: D, read_only: bool) -> Result<Self> {
        let reader = BlockReader::new(&device);
        let sb_manager = SuperBlockManager::load(&reader)?;
        let bg_manager = BlockGroupManager::load(&reader, &sb_manager)?;

        let (block_allocator, inode_allocator) = if read_only {
            (None, None)
        } else {
            let mut block_groups = Vec::with_capacity(sb_manager.group_count as usize);
            let mut inode_groups = Vec::with_capacity(sb_manager.group_count as usize);
            let mut buf = vec![0u8; sb_manager.block_size];
            let is_64bit = sb_manager.is_64bit;

            let block_bits = sb_manager.super_block.s_blocks_per_group as usize;
            let block_bitmap_bytes = block_bits.div_ceil(8);
            let inode_bits = sb_manager.super_block.s_inodes_per_group as usize;
            let inode_bitmap_bytes = inode_bits.div_ceil(8);

            for g in 0..bg_manager.count() {
                let desc = bg_manager.get_desc(g);

                reader.read_block(bg_manager.block_bitmap_block(g), &mut buf)?;
                block_groups.push(BlockGroupAllocState {
                    block_bitmap: buf[..block_bitmap_bytes].to_vec(),
                    free_blocks_count: desc.free_blocks_count(is_64bit),
                    max_bits: block_bits,
                });

                reader.read_block(bg_manager.inode_bitmap_block(g), &mut buf)?;
                inode_groups.push(InodeGroupAllocState {
                    inode_bitmap: buf[..inode_bitmap_bytes].to_vec(),
                    free_inodes_count: desc.free_inodes_count(is_64bit),
                    free_blocks_count: desc.free_blocks_count(is_64bit),
                    used_dirs_count: desc.used_dirs_count(is_64bit),
                    max_bits: inode_bits,
                });
            }

            (
                Some(Ext4BlockAllocator::new(
                    sb_manager.super_block.s_first_data_block as u64,
                    sb_manager.super_block.s_blocks_per_group,
                    block_groups,
                )),
                Some(Ext4InodeAllocator::new(
                    sb_manager.super_block.s_inodes_per_group,
                    inode_groups,
                )),
            )
        };

        let journal = if read_only {
            None
        } else {
            // Try to load journal from s_journal_inum (usually inode 8).
            Self::try_load_journal_inode(&reader, &sb_manager, &bg_manager)
                .or_else(|| Self::synthesize_journal(&sb_manager))
        };

        let mut fs = Self {
            device,
            sb_manager,
            bg_manager,
            read_only,
            block_allocator,
            inode_allocator,
            journal,
            dirty_blocks: BTreeSet::new(),
        };

        // Clean up orphan inodes left over from a crash.
        if !read_only {
            fs.cleanup_orphans()?;
        }

        Ok(fs)
    }

    /// Attempt to load the journal from the inode pointed to by `s_journal_inum`.
    /// Returns `None` if the journal inode cannot be read or has no extent data.
    fn try_load_journal_inode<R: BlockDevice>(
        reader: &BlockReader<R>,
        sb_mgr: &SuperBlockManager,
        bg_mgr: &BlockGroupManager,
    ) -> Option<FsJournalState> {
        use crate::fs_core::extent_walker::ExtentWalker;

        let j_inum = sb_mgr.super_block.s_journal_inum;
        if j_inum == 0 {
            return None;
        }
        let inode = InodeReader::read_inode(reader, sb_mgr, bg_mgr, j_inum).ok()?;
        // The first logical block (block 0) of the journal inode holds the journal superblock.
        let mapping = ExtentWalker::logical_to_physical(reader, sb_mgr, &inode, 0).ok()??;
        let start_block = mapping.physical_block;
        // Read the journal superblock from the first block.
        let mut buf = vec![0u8; sb_mgr.block_size];
        reader.read_block(start_block, &mut buf).ok()?;
        let jsb = JournalSuperBlock::parse(&buf).ok()?;
        Some(FsJournalState {
            start_block,
            superblock: jsb.clone(),
            has_64bit: sb_mgr.is_64bit,
            has_csum: sb_mgr.has_metadata_csum,
        })
    }

    /// Synthesize a minimal journal at the end of the device when no real journal inode is available.
    fn synthesize_journal(sb_mgr: &SuperBlockManager) -> Option<FsJournalState> {
        let total_blocks = sb_mgr.super_block.block_count();
        let journal_len = core::cmp::max(8u64, core::cmp::min(64u64, total_blocks / 4));
        if journal_len + 1 >= total_blocks {
            return None;
        }
        let start_block = total_blocks - journal_len;
        Some(FsJournalState {
            start_block,
            superblock: JournalSuperBlock {
                header: JournalHeader {
                    h_magic: JBD2_MAGIC_NUMBER,
                    h_blocktype: JBD2_BLOCKTYPE_SUPERBLOCK_V2,
                    h_sequence: 1,
                },
                s_blocksize: sb_mgr.block_size as u32,
                s_maxlen: journal_len as u32,
                s_first: 1,
                s_sequence: 1,
                s_start: 1,
                s_errno: 0,
                s_feature_compat: 0,
                s_feature_incompat: 0,
                s_feature_ro_compat: 0,
                s_uuid: sb_mgr.super_block.s_uuid,
                s_nr_users: 1,
                s_checksum_type: 0,
                s_checksum: 0,
            },
            has_64bit: sb_mgr.is_64bit,
            has_csum: sb_mgr.has_metadata_csum,
        })
    }

    pub fn resolve_path(&self, path: &str) -> Result<u32> {
        let reader = BlockReader::new(&self.device);
        PathResolver::resolve(&reader, &self.sb_manager, &self.bg_manager, path)
    }

    pub fn resolve_parent(&self, path: &str) -> Result<(u32, alloc::string::String)> {
        let reader = BlockReader::new(&self.device);
        PathResolver::resolve_parent(&reader, &self.sb_manager, &self.bg_manager, path)
    }

    pub fn read_inode(&self, ino: u32) -> Result<Inode> {
        let reader = BlockReader::new(&self.device);
        InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)
    }

    pub fn read_symlink(&self, ino: u32) -> Result<alloc::string::String> {
        let reader = BlockReader::new(&self.device);
        let inode = InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)?;
        SymlinkReader::read_symlink(&reader, &self.sb_manager, &inode)
    }

    pub fn journal_sequence(&self) -> Option<u32> {
        self.journal.as_ref().map(|j| j.superblock.s_sequence)
    }

    /// Compute the physical block number that contains the given inode's on-disk data.
    fn inode_phys_block(&self, ino: u32) -> u64 {
        let sb = &self.sb_manager.super_block;
        let inode_size = sb.s_inode_size as u64;
        let block_size = self.sb_manager.block_size as u64;
        let group = (ino - 1) / sb.s_inodes_per_group;
        let index = (ino - 1) % sb.s_inodes_per_group;
        let table_block = self.bg_manager.inode_table_block(group);
        let byte_offset = table_block * block_size + index as u64 * inode_size;
        byte_offset / block_size
    }

    /// Record that the inode table block for the given inode was dirtied.
    fn track_inode_dirty(&mut self, ino: u32) {
        let blk = self.inode_phys_block(ino);
        self.dirty_blocks.insert(blk);
    }

    /// Record that a specific block was dirtied (directory data, extent tree node, etc.).
    #[allow(dead_code)]
    fn track_block_dirty(&mut self, blk: u64) {
        self.dirty_blocks.insert(blk);
    }

    /// Walk the orphan inode list and delete/truncate each orphaned inode.
    ///
    /// The list is a linked list through `i_dtime`: `s_last_orphan` → inode.i_dtime → … → 0.
    /// Orphans with `i_links_count == 0` are fully freed; others are truncated to `i_size`.
    fn cleanup_orphans(&mut self) -> Result<()> {
        let mut ino = self.sb_manager.super_block.s_last_orphan;
        if ino == 0 {
            return Ok(());
        }

        while ino != 0 {
            let inode = match self.read_inode(ino) {
                Ok(i) => i,
                Err(_) => break, // broken chain, stop
            };
            let next = inode.i_dtime; // next orphan inode number

            if inode.i_links_count == 0 {
                // Fully free: release all data blocks and the inode itself.
                let mut block_allocator = match self.block_allocator.take() {
                    Some(ba) => ba,
                    None => break,
                };
                let mut inode_allocator = match self.inode_allocator.take() {
                    Some(ia) => ia,
                    None => {
                        self.block_allocator = Some(block_allocator);
                        break;
                    }
                };
                let mut dead_inode = inode.clone();
                let result = (|| -> Result<()> {
                    let mut writer = BlockWriter::new(&mut self.device);
                    let removed = ExtentModifier::remove_extents(
                        &mut writer,
                        &self.sb_manager,
                        &mut dead_inode,
                        0,
                        &mut block_allocator,
                    )?;
                    let mut pblks = Vec::new();
                    for (start, count) in removed {
                        for i in 0..count {
                            pblks.push(start + i as u64);
                        }
                    }
                    if !pblks.is_empty() {
                        block_allocator.free_blocks(&pblks)?;
                    }
                    inode_allocator.free_inode(ino)?;
                    Ok(())
                })();
                self.block_allocator = Some(block_allocator);
                self.inode_allocator = Some(inode_allocator);
                result?;
            }
            // else: orphan with links_count > 0 means interrupted truncate.
            // For now, we just remove it from the orphan list. A full implementation
            // would truncate to i_size here.

            ino = next;
        }

        // Clear s_last_orphan in superblock.
        self.sb_manager.super_block.s_last_orphan = 0;
        Ok(())
    }

    /// Add an inode to the orphan linked list (prepend to s_last_orphan).
    /// Sets inode.i_dtime = old s_last_orphan, then s_last_orphan = ino.
    fn orphan_add(&mut self, ino: u32) -> Result<()> {
        let prev_head = self.sb_manager.super_block.s_last_orphan;
        let mut inode = self.read_inode(ino)?;
        inode.i_dtime = prev_head;
        let mut writer = BlockWriter::new(&mut self.device);
        InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
        self.sb_manager.super_block.s_last_orphan = ino;
        Ok(())
    }

    /// Remove an inode from the orphan linked list.
    /// Walks the list from s_last_orphan to find and unlink ino.
    fn orphan_remove(&mut self, ino: u32) -> Result<()> {
        if self.sb_manager.super_block.s_last_orphan == 0 {
            return Ok(());
        }
        if self.sb_manager.super_block.s_last_orphan == ino {
            let inode = self.read_inode(ino)?;
            self.sb_manager.super_block.s_last_orphan = inode.i_dtime;
            // Clear i_dtime.
            let mut cleared = inode;
            cleared.i_dtime = 0;
            let mut writer = BlockWriter::new(&mut self.device);
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                ino,
                &cleared,
            )?;
            return Ok(());
        }
        // Walk the chain.
        let mut cur = self.sb_manager.super_block.s_last_orphan;
        while cur != 0 {
            let cur_inode = self.read_inode(cur)?;
            if cur_inode.i_dtime == ino {
                let target = self.read_inode(ino)?;
                // Unlink: cur.i_dtime = target.i_dtime
                let mut patched = cur_inode.clone();
                patched.i_dtime = target.i_dtime;
                let mut writer = BlockWriter::new(&mut self.device);
                InodeWriter::write_inode(
                    &mut writer,
                    &self.sb_manager,
                    &self.bg_manager,
                    cur,
                    &patched,
                )?;
                // Clear target i_dtime.
                let mut cleared = target;
                cleared.i_dtime = 0;
                InodeWriter::write_inode(
                    &mut writer,
                    &self.sb_manager,
                    &self.bg_manager,
                    ino,
                    &cleared,
                )?;
                return Ok(());
            }
            cur = cur_inode.i_dtime;
        }
        Ok(()) // not found, silently ignore
    }

    fn journal_commit_tick(&mut self) -> Result<()> {
        // Ensure all allocator metadata (bitmaps, GDT, superblock) is flushed first.
        self.flush_alloc_metadata()?;

        let Some(mut state) = self.journal.take() else {
            return Ok(());
        };

        // Drain dirty blocks collected during the current operation.
        let blocks: Vec<u64> = self.dirty_blocks.iter().copied().collect();
        self.dirty_blocks.clear();

        // If no actual dirty blocks were recorded, still commit a heartbeat (block 0)
        // so the journal sequence always advances.
        let mut journal = Jbd2Journal::new(
            &mut self.device,
            state.start_block,
            state.superblock.clone(),
            state.has_64bit,
            state.has_csum,
        );
        let h = journal.start_transaction()?;
        if blocks.is_empty() {
            journal.get_write_access(&h, 0)?;
            journal.dirty_metadata(&h, 0)?;
        } else {
            for &blk in &blocks {
                journal.get_write_access(&h, blk)?;
                journal.dirty_metadata(&h, blk)?;
            }
        }
        journal.commit(h)?;
        state.superblock = journal.journal_superblock().clone();
        self.journal = Some(state);
        Ok(())
    }

    /// Write back all dirty block/inode bitmaps, update GDT entries on disk,
    /// and update superblock free counts.
    fn flush_alloc_metadata(&mut self) -> Result<()> {
        let mut writer = BlockWriter::new(&mut self.device);
        let desc_size = self.sb_manager.desc_size as usize;
        let block_size = self.sb_manager.block_size;
        let is_64bit = self.sb_manager.is_64bit;
        let has_csum = self.sb_manager.has_metadata_csum;
        let csum_seed = self.sb_manager.csum_seed;
        let desc_table_start = BlockGroupManager::desc_table_start(block_size);

        // Flush dirty block bitmaps.
        if let Some(ref mut ba) = self.block_allocator {
            let dirty = ba.drain_dirty_groups();
            for g in dirty {
                let bitmap_block = self.bg_manager.block_bitmap_block(g as u32);
                let bitmap = ba.group_bitmap(g);
                // Pad bitmap to full block size.
                let mut buf = vec![0u8; block_size];
                let copy_len = bitmap.len().min(block_size);
                buf[..copy_len].copy_from_slice(&bitmap[..copy_len]);
                writer.write_block(bitmap_block, &buf)?;

                // Update GDT in-memory.
                let desc = self.bg_manager.get_desc_mut(g as u32);
                desc.set_free_blocks_count(ba.group_free_count(g), is_64bit);
                // Serialize and compute checksum.
                let mut raw = desc.serialize(desc_size, is_64bit);
                if has_csum {
                    let csum = block_group_checksum(csum_seed, g as u32, &raw);
                    raw[0x1E..0x20].copy_from_slice(&csum.to_le_bytes());
                    desc.bg_checksum = csum;
                }
                // Write the descriptor back.
                let desc_byte_offset =
                    desc_table_start * block_size as u64 + g as u64 * desc_size as u64;
                writer.write_bytes(desc_byte_offset, &raw)?;
            }
        }

        // Flush dirty inode bitmaps.
        if let Some(ref mut ia) = self.inode_allocator {
            let dirty = ia.drain_dirty_groups();
            for g in dirty {
                let bitmap_block = self.bg_manager.inode_bitmap_block(g as u32);
                let bitmap = ia.group_bitmap(g);
                let mut buf = vec![0u8; block_size];
                let copy_len = bitmap.len().min(block_size);
                buf[..copy_len].copy_from_slice(&bitmap[..copy_len]);
                writer.write_block(bitmap_block, &buf)?;

                // Update GDT in-memory.
                let desc = self.bg_manager.get_desc_mut(g as u32);
                desc.set_free_inodes_count(ia.group_free_count(g), is_64bit);
                desc.set_used_dirs_count(ia.group_used_dirs(g), is_64bit);
                let mut raw = desc.serialize(desc_size, is_64bit);
                if has_csum {
                    let csum = block_group_checksum(csum_seed, g as u32, &raw);
                    raw[0x1E..0x20].copy_from_slice(&csum.to_le_bytes());
                    desc.bg_checksum = csum;
                }
                let desc_byte_offset =
                    desc_table_start * block_size as u64 + g as u64 * desc_size as u64;
                writer.write_bytes(desc_byte_offset, &raw)?;
            }
        }

        // Update superblock free counts.
        let free_blocks = self
            .block_allocator
            .as_ref()
            .map_or(0, |ba| ba.free_blocks_total);
        let free_inodes = self
            .inode_allocator
            .as_ref()
            .map_or(0, |ia| ia.free_inodes_total);
        self.sb_manager.super_block.s_free_blocks_count_lo = free_blocks as u32;
        self.sb_manager.super_block.s_free_blocks_count_hi = (free_blocks >> 32) as u32;
        self.sb_manager.super_block.s_free_inodes_count = free_inodes as u32;

        // Write superblock back at byte offset 1024.
        use crate::layout::checksum::superblock_checksum;
        use crate::layout::superblock::{SUPER_BLOCK_OFFSET, SUPER_BLOCK_SIZE};
        let mut sb_raw = [0u8; SUPER_BLOCK_SIZE];
        // Read current superblock, update free counts, recompute checksum.
        writer
            .device()
            .read_block(
                SUPER_BLOCK_OFFSET as u64 / block_size as u64,
                &mut vec![0u8; block_size],
            )
            .ok(); // ignore if this fails on tiny test devices
        let bs = block_size;
        let sb_block = if bs == 1024 { 1u64 } else { 0u64 };
        let sb_offset_in_block = if bs == 1024 { 0 } else { SUPER_BLOCK_OFFSET };
        let mut full_block = vec![0u8; bs];
        writer.device().read_block(sb_block, &mut full_block)?;
        // Copy current sb out
        sb_raw.copy_from_slice(
            &full_block[sb_offset_in_block..sb_offset_in_block + SUPER_BLOCK_SIZE],
        );
        // Update free counts and orphan head in raw bytes.
        sb_raw[0x0C..0x10].copy_from_slice(&(free_blocks as u32).to_le_bytes());
        sb_raw[0x10..0x14].copy_from_slice(&(free_inodes as u32).to_le_bytes());
        sb_raw[0x158..0x15C].copy_from_slice(&((free_blocks >> 32) as u32).to_le_bytes());
        sb_raw[0xE8..0xEC]
            .copy_from_slice(&self.sb_manager.super_block.s_last_orphan.to_le_bytes());
        // Recompute checksum if enabled.
        if has_csum {
            let csum = superblock_checksum(&sb_raw);
            sb_raw[0x3FC..0x400].copy_from_slice(&csum.to_le_bytes());
        }
        full_block[sb_offset_in_block..sb_offset_in_block + SUPER_BLOCK_SIZE]
            .copy_from_slice(&sb_raw);
        writer.write_block(sb_block, &full_block)?;

        Ok(())
    }
}

impl<D: BlockDevice> FileSystem for Ext4FileSystem<D> {
    fn unmount(&mut self) -> Result<()> {
        self.journal_commit_tick()?;
        self.device.flush()
    }

    fn sync(&mut self) -> Result<()> {
        self.journal_commit_tick()?;
        self.device.flush()
    }

    fn stat_fs(&self) -> Result<StatFs> {
        let sb = &self.sb_manager.super_block;
        Ok(StatFs {
            block_size: self.sb_manager.block_size as u64,
            total_blocks: sb.block_count(),
            free_blocks: sb.free_blocks_count(),
            total_inodes: sb.s_inodes_count as u64,
            free_inodes: sb.s_free_inodes_count as u64,
        })
    }
}

impl<D: BlockDevice> InodeOps for Ext4FileSystem<D> {
    fn lookup(&self, parent: u32, name: &str) -> Result<u32> {
        let reader = BlockReader::new(&self.device);
        let parent_inode =
            InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;
        DirReader::lookup(&reader, &self.sb_manager, &parent_inode, name)
    }

    fn read(&self, ino: u32, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let reader = BlockReader::new(&self.device);
        let inode = InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)?;
        FileReader::read(&reader, &self.sb_manager, &inode, offset, buf)
    }

    fn readdir(&self, ino: u32) -> Result<Vec<crate::layout::dir_entry::DirEntry>> {
        let reader = BlockReader::new(&self.device);
        let inode = InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, ino)?;
        DirReader::read_dir_entries(&reader, &self.sb_manager, &inode)
    }

    fn create(&mut self, parent: u32, name: &str, mode: u16, uid: u32, gid: u32) -> Result<u32> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        if self.lookup(parent, name).is_ok() {
            return Err(Ext4Error::CorruptedFs("entry already exists"));
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<u32> {
            let mut writer = BlockWriter::new(&mut self.device);
            let (new_ino, new_inode) =
                InodeWriter::alloc_and_init_inode(&mut inode_allocator, parent, mode, uid, gid)?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                new_ino,
                &new_inode,
            )?;

            let reader = writer.as_reader();
            let mut parent_inode =
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;
            DirWriter::add_entry(
                &mut writer,
                &self.sb_manager,
                &mut parent_inode,
                name,
                new_ino,
                DirEntryFileType::RegularFile,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;
            Ok(new_ino)
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        if let Ok(new_ino) = result {
            self.track_inode_dirty(new_ino);
            self.track_inode_dirty(parent);
            self.journal_commit_tick()?;
            return Ok(new_ino);
        }
        result
    }

    fn write(&mut self, ino: u32, offset: u64, data: &[u8]) -> Result<usize> {
        let mut inode = self.read_inode(ino)?;
        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<usize> {
            let mut writer = BlockWriter::new(&mut self.device);
            let n = FileWriter::write(
                &mut writer,
                &self.sb_manager,
                &mut inode,
                offset,
                data,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
            Ok(n)
        })();
        self.block_allocator = Some(block_allocator);
        if let Ok(n) = result {
            self.track_inode_dirty(ino);
            self.journal_commit_tick()?;
            return Ok(n);
        }
        result
    }

    fn unlink(&mut self, parent: u32, name: &str) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        // Resolve the target inode before removing so we can add it to orphan list.
        let target_ino = self.lookup(parent, name)?;
        // Add to orphan list BEFORE removing the directory entry.
        // If we crash between orphan_add and dir removal, recovery will clean it up.
        self.orphan_add(target_ino)?;

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let removed_ino_cell: core::cell::Cell<u32> = core::cell::Cell::new(0);
        let result = (|| -> Result<()> {
            let mut writer = BlockWriter::new(&mut self.device);
            let parent_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?
            };
            let removed_ino =
                DirWriter::remove_entry(&mut writer, &self.sb_manager, &parent_inode, name)?;
            removed_ino_cell.set(removed_ino);
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;

            let mut removed_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, removed_ino)?
            };
            if removed_inode.i_links_count > 0 {
                removed_inode.i_links_count -= 1;
            }
            if removed_inode.i_links_count == 0 {
                let removed = ExtentModifier::remove_extents(
                    &mut writer,
                    &self.sb_manager,
                    &mut removed_inode,
                    0,
                    &mut block_allocator,
                )?;
                let mut pblks = Vec::new();
                for (start, count) in removed {
                    for i in 0..count {
                        pblks.push(start + i as u64);
                    }
                }
                if !pblks.is_empty() {
                    block_allocator.free_blocks(&pblks)?;
                }
                inode_allocator.free_inode(removed_ino)?;
            } else {
                InodeWriter::write_inode(
                    &mut writer,
                    &self.sb_manager,
                    &self.bg_manager,
                    removed_ino,
                    &removed_inode,
                )?;
            }
            Ok(())
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        if result.is_ok() {
            // Successfully unlinked — remove from orphan list.
            self.orphan_remove(target_ino)?;
            self.track_inode_dirty(parent);
            let rino = removed_ino_cell.get();
            if rino != 0 {
                self.track_inode_dirty(rino);
            }
            self.journal_commit_tick()?;
        }
        result
    }

    fn mkdir(&mut self, parent: u32, name: &str, mode: u16, uid: u32, gid: u32) -> Result<u32> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        if self.lookup(parent, name).is_ok() {
            return Err(Ext4Error::CorruptedFs("entry already exists"));
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<u32> {
            let mut writer = BlockWriter::new(&mut self.device);
            let mode = (mode & 0x0FFF) | S_IFDIR;
            let (new_ino, new_inode) =
                InodeWriter::alloc_and_init_inode(&mut inode_allocator, parent, mode, uid, gid)?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                new_ino,
                &new_inode,
            )?;

            let reader = writer.as_reader();
            let mut parent_inode =
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;
            DirWriter::add_entry(
                &mut writer,
                &self.sb_manager,
                &mut parent_inode,
                name,
                new_ino,
                DirEntryFileType::Directory,
                &mut block_allocator,
            )?;
            parent_inode.i_links_count = parent_inode.i_links_count.saturating_add(1);
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;
            Ok(new_ino)
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        if let Ok(new_ino) = result {
            self.track_inode_dirty(new_ino);
            self.track_inode_dirty(parent);
            self.journal_commit_tick()?;
            return Ok(new_ino);
        }
        result
    }

    fn rmdir(&mut self, parent: u32, name: &str) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let target_ino_cell: core::cell::Cell<u32> = core::cell::Cell::new(0);
        let result = (|| -> Result<()> {
            let mut writer = BlockWriter::new(&mut self.device);
            let parent_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?
            };
            let target_ino = {
                let reader = writer.as_reader();
                DirReader::lookup(&reader, &self.sb_manager, &parent_inode, name)?
            };
            target_ino_cell.set(target_ino);
            let target_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, target_ino)?
            };
            if !target_inode.is_dir() {
                return Err(Ext4Error::NotDirectory);
            }

            let entries = {
                let reader = writer.as_reader();
                DirReader::read_dir_entries(&reader, &self.sb_manager, &target_inode)?
            };
            let mut non_dot = 0usize;
            for e in entries {
                if !e.is_dot_or_dotdot() {
                    non_dot += 1;
                }
            }
            if non_dot != 0 {
                return Err(Ext4Error::CorruptedFs("directory not empty"));
            }

            let removed_ino =
                DirWriter::remove_entry(&mut writer, &self.sb_manager, &parent_inode, name)?;
            if removed_ino != target_ino {
                return Err(Ext4Error::CorruptedFs("removed inode mismatch"));
            }

            let mut removed_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, removed_ino)?
            };
            let removed = ExtentModifier::remove_extents(
                &mut writer,
                &self.sb_manager,
                &mut removed_inode,
                0,
                &mut block_allocator,
            )?;
            let mut pblks = Vec::new();
            for (start, count) in removed {
                for i in 0..count {
                    pblks.push(start + i as u64);
                }
            }
            if !pblks.is_empty() {
                block_allocator.free_blocks(&pblks)?;
            }
            inode_allocator.free_inode(removed_ino)?;
            Ok(())
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        if result.is_ok() {
            self.track_inode_dirty(parent);
            let tino = target_ino_cell.get();
            if tino != 0 {
                self.track_inode_dirty(tino);
            }
            self.journal_commit_tick()?;
        }
        result
    }

    fn rename(
        &mut self,
        old_parent: u32,
        old_name: &str,
        new_parent: u32,
        new_name: &str,
    ) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        if self.lookup(new_parent, new_name).is_ok() {
            return Err(Ext4Error::CorruptedFs("rename target exists"));
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<()> {
            let mut writer = BlockWriter::new(&mut self.device);
            let mut old_parent_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, old_parent)?
            };
            let moved_ino = DirWriter::remove_entry(
                &mut writer,
                &self.sb_manager,
                &old_parent_inode,
                old_name,
            )?;
            let moved_inode = {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, moved_ino)?
            };

            let is_dir = moved_inode.is_dir();
            let cross_parent = old_parent != new_parent;

            let mut new_parent_inode = if cross_parent {
                let reader = writer.as_reader();
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, new_parent)?
            } else {
                old_parent_inode.clone()
            };

            let ftype = match moved_inode.file_type() {
                InodeFileType::Directory => DirEntryFileType::Directory,
                InodeFileType::Symlink => DirEntryFileType::Symlink,
                _ => DirEntryFileType::RegularFile,
            };
            DirWriter::add_entry(
                &mut writer,
                &self.sb_manager,
                &mut new_parent_inode,
                new_name,
                moved_ino,
                ftype,
                &mut block_allocator,
            )?;

            // When moving a subdirectory across different parents, adjust i_links_count:
            //   old_parent loses ".." reference → decrement
            //   new_parent gains ".." reference → increment
            if is_dir && cross_parent {
                if old_parent_inode.i_links_count > 0 {
                    old_parent_inode.i_links_count -= 1;
                }
                new_parent_inode.i_links_count = new_parent_inode.i_links_count.saturating_add(1);
            }

            // Write back parent inodes if they were modified.
            if cross_parent {
                InodeWriter::write_inode(
                    &mut writer,
                    &self.sb_manager,
                    &self.bg_manager,
                    old_parent,
                    &old_parent_inode,
                )?;
                InodeWriter::write_inode(
                    &mut writer,
                    &self.sb_manager,
                    &self.bg_manager,
                    new_parent,
                    &new_parent_inode,
                )?;
            }
            Ok(())
        })();
        self.block_allocator = Some(block_allocator);
        if result.is_ok() {
            self.track_inode_dirty(old_parent);
            if old_parent != new_parent {
                self.track_inode_dirty(new_parent);
            }
            self.journal_commit_tick()?;
        }
        result
    }

    fn truncate(&mut self, ino: u32, new_size: u64) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        let mut inode = self.read_inode(ino)?;
        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<()> {
            let mut writer = BlockWriter::new(&mut self.device);
            FileWriter::truncate(
                &mut writer,
                &self.sb_manager,
                &mut inode,
                new_size,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
            Ok(())
        })();
        self.block_allocator = Some(block_allocator);
        if result.is_ok() {
            self.track_inode_dirty(ino);
            self.journal_commit_tick()?;
        }
        result
    }

    fn symlink(
        &mut self,
        parent: u32,
        name: &str,
        target: &str,
        uid: u32,
        gid: u32,
    ) -> Result<u32> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        if self.lookup(parent, name).is_ok() {
            return Err(Ext4Error::CorruptedFs("entry already exists"));
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let mut inode_allocator = self.inode_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<u32> {
            let mut writer = BlockWriter::new(&mut self.device);
            let mode = S_IFLNK | 0o777;
            let (new_ino, mut link_inode) =
                InodeWriter::alloc_and_init_inode(&mut inode_allocator, parent, mode, uid, gid)?;

            if target.len() <= link_inode.i_block.len() {
                link_inode.i_size = target.len() as u64;
                link_inode.i_blocks = 0;
                link_inode.i_block.fill(0);
                link_inode.i_block[..target.len()].copy_from_slice(target.as_bytes());
            } else {
                FileWriter::write(
                    &mut writer,
                    &self.sb_manager,
                    &mut link_inode,
                    0,
                    target.as_bytes(),
                    &mut block_allocator,
                )?;
            }

            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                new_ino,
                &link_inode,
            )?;

            let reader = writer.as_reader();
            let mut parent_inode =
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;
            DirWriter::add_entry(
                &mut writer,
                &self.sb_manager,
                &mut parent_inode,
                name,
                new_ino,
                DirEntryFileType::Symlink,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;
            Ok(new_ino)
        })();
        self.block_allocator = Some(block_allocator);
        self.inode_allocator = Some(inode_allocator);
        if let Ok(new_ino) = result {
            self.track_inode_dirty(new_ino);
            self.track_inode_dirty(parent);
            self.journal_commit_tick()?;
            return Ok(new_ino);
        }
        result
    }

    fn readlink(&self, ino: u32) -> Result<String> {
        self.read_symlink(ino)
    }

    fn stat(&self, ino: u32) -> Result<Inode> {
        self.read_inode(ino)
    }

    fn chmod(&mut self, ino: u32, mode: u16) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        let mut inode = self.read_inode(ino)?;
        // Preserve file type bits, only change permission bits.
        inode.i_mode = (inode.i_mode & 0xF000) | (mode & 0x0FFF);
        let mut writer = BlockWriter::new(&mut self.device);
        InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
        self.track_inode_dirty(ino);
        self.journal_commit_tick()
    }

    fn chown(&mut self, ino: u32, uid: u32, gid: u32) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        let mut inode = self.read_inode(ino)?;
        inode.i_uid = uid;
        inode.i_gid = gid;
        let mut writer = BlockWriter::new(&mut self.device);
        InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
        self.track_inode_dirty(ino);
        self.journal_commit_tick()
    }

    fn utimes(&mut self, ino: u32, atime: u32, mtime: u32) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        let mut inode = self.read_inode(ino)?;
        inode.i_atime = atime;
        inode.i_mtime = mtime;
        let mut writer = BlockWriter::new(&mut self.device);
        InodeWriter::write_inode(&mut writer, &self.sb_manager, &self.bg_manager, ino, &inode)?;
        self.track_inode_dirty(ino);
        self.journal_commit_tick()
    }

    fn link(&mut self, parent: u32, name: &str, ino: u32) -> Result<()> {
        if self.read_only {
            return Err(Ext4Error::ReadOnly);
        }
        if self.lookup(parent, name).is_ok() {
            return Err(Ext4Error::CorruptedFs("entry already exists"));
        }
        // Hard links to directories are not allowed.
        let mut target_inode = self.read_inode(ino)?;
        if target_inode.is_dir() {
            return Err(Ext4Error::CorruptedFs("hard link to directory not allowed"));
        }

        let mut block_allocator = self.block_allocator.take().ok_or(Ext4Error::ReadOnly)?;
        let result = (|| -> Result<()> {
            let mut writer = BlockWriter::new(&mut self.device);
            let reader = writer.as_reader();
            let mut parent_inode =
                InodeReader::read_inode(&reader, &self.sb_manager, &self.bg_manager, parent)?;

            let ftype = match target_inode.file_type() {
                InodeFileType::Symlink => DirEntryFileType::Symlink,
                _ => DirEntryFileType::RegularFile,
            };
            DirWriter::add_entry(
                &mut writer,
                &self.sb_manager,
                &mut parent_inode,
                name,
                ino,
                ftype,
                &mut block_allocator,
            )?;
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                parent,
                &parent_inode,
            )?;

            // Increment target inode link count.
            target_inode.i_links_count = target_inode.i_links_count.saturating_add(1);
            InodeWriter::write_inode(
                &mut writer,
                &self.sb_manager,
                &self.bg_manager,
                ino,
                &target_inode,
            )?;
            Ok(())
        })();
        self.block_allocator = Some(block_allocator);
        if result.is_ok() {
            self.track_inode_dirty(parent);
            self.track_inode_dirty(ino);
            self.journal_commit_tick()?;
        }
        result
    }
}
