use alloc::vec;

use crate::error::{Ext4Error, Result};
use crate::fs_core::block_group_manager::BlockGroupManager;
use crate::fs_core::extent_modifier::ExtentModifier;
use crate::fs_core::superblock_manager::SuperBlockManager;
use crate::io::block_writer::BlockWriter;
use crate::layout::checksum::inode_checksum;
use crate::layout::inode::{EXTENTS_FL, Inode, S_IFDIR};
use crate::traits::allocator::InodeAllocator;
use crate::traits::block_device::BlockDevice;

/// Maximum supported inode size (stack buffer limit).
const MAX_INODE_SIZE: usize = 1024;

pub struct InodeWriter;

impl InodeWriter {
    pub fn write_inode<D: BlockDevice>(
        writer: &mut BlockWriter<D>,
        super_block_manager: &SuperBlockManager,
        block_group_manager: &BlockGroupManager,
        ino: u32,
        inode: &Inode,
    ) -> Result<()> {
        let sb = &super_block_manager.super_block;
        if ino == 0 || ino > sb.s_inodes_count {
            return Err(Ext4Error::CorruptedFs("inode number out of range"));
        }

        let inode_size = sb.s_inode_size as usize;
        let block_size = super_block_manager.block_size;
        if inode_size > MAX_INODE_SIZE || inode_size > block_size {
            return Err(Ext4Error::CorruptedFs("inode_size exceeds supported limit"));
        }

        let inodes_per_group = sb.s_inodes_per_group;
        let group = (ino - 1) / inodes_per_group;
        let index = (ino - 1) % inodes_per_group;
        let table_block = block_group_manager.inode_table_block(group);
        let byte_offset = table_block * block_size as u64 + index as u64 * inode_size as u64;
        let block_no = byte_offset / block_size as u64;
        let offset_in_block = (byte_offset % block_size as u64) as usize;

        let mut block_buf = vec![0u8; block_size];
        writer.device().read_block(block_no, &mut block_buf)?;

        let mut inode_raw = [0u8; MAX_INODE_SIZE];
        Self::serialize_inode(inode, sb.s_inode_size, &mut inode_raw[..inode_size])?;

        // Compute and fill inode checksum if metadata checksumming is enabled.
        if super_block_manager.has_metadata_csum {
            let csum = inode_checksum(
                super_block_manager.csum_seed,
                ino,
                inode.i_generation,
                &inode_raw[..inode_size],
            );
            // Write i_checksum_lo at 0x7C..0x7E
            inode_raw[0x7C..0x7E].copy_from_slice(&(csum as u16).to_le_bytes());
            // Write i_checksum_hi at 0x82..0x84 if inode has extended fields
            if inode_size > 128 {
                inode_raw[0x82..0x84].copy_from_slice(&((csum >> 16) as u16).to_le_bytes());
            }
        }

        block_buf[offset_in_block..offset_in_block + inode_size]
            .copy_from_slice(&inode_raw[..inode_size]);
        writer.write_block(block_no, &block_buf)?;
        Ok(())
    }

    pub fn alloc_and_init_inode<A: InodeAllocator>(
        inode_allocator: &mut A,
        parent_ino: u32,
        mode: u16,
        uid: u32,
        gid: u32,
    ) -> Result<(u32, Inode)> {
        let is_dir = mode & 0xF000 == S_IFDIR;
        let ino = inode_allocator.alloc_inode(parent_ino, is_dir)?;

        let mut inode = Inode {
            i_mode: mode,
            i_uid: uid,
            i_gid: gid,
            i_size: 0,
            i_atime: 0,
            i_ctime: 0,
            i_mtime: 0,
            i_dtime: 0,
            i_links_count: if is_dir { 2 } else { 1 },
            i_blocks: 0,
            i_flags: EXTENTS_FL,
            i_block: [0u8; 60],
            i_generation: 0,
            i_file_acl: 0,
            i_extra_isize: 0,
            i_checksum: 0,
        };
        ExtentModifier::init_empty_extent_root(&mut inode);
        Ok((ino, inode))
    }

    fn serialize_inode(inode: &Inode, inode_size: u16, out: &mut [u8]) -> Result<()> {
        let isize = inode_size as usize;
        if out.len() < isize || isize < 128 {
            return Err(Ext4Error::CorruptedFs("inode serialize buffer too small"));
        }

        out[..isize].fill(0);
        out[0x00..0x02].copy_from_slice(&inode.i_mode.to_le_bytes());
        out[0x02..0x04].copy_from_slice(&(inode.i_uid as u16).to_le_bytes());
        out[0x04..0x08].copy_from_slice(&(inode.i_size as u32).to_le_bytes());
        out[0x08..0x0C].copy_from_slice(&inode.i_atime.to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&inode.i_ctime.to_le_bytes());
        out[0x10..0x14].copy_from_slice(&inode.i_mtime.to_le_bytes());
        out[0x14..0x18].copy_from_slice(&inode.i_dtime.to_le_bytes());
        out[0x18..0x1A].copy_from_slice(&(inode.i_gid as u16).to_le_bytes());
        out[0x1A..0x1C].copy_from_slice(&inode.i_links_count.to_le_bytes());
        out[0x1C..0x20].copy_from_slice(&(inode.i_blocks as u32).to_le_bytes());
        out[0x20..0x24].copy_from_slice(&inode.i_flags.to_le_bytes());
        out[0x28..0x64].copy_from_slice(&inode.i_block);
        out[0x64..0x68].copy_from_slice(&inode.i_generation.to_le_bytes());
        out[0x68..0x6C].copy_from_slice(&(inode.i_file_acl as u32).to_le_bytes());
        out[0x6C..0x70].copy_from_slice(&((inode.i_size >> 32) as u32).to_le_bytes());
        out[0x74..0x76].copy_from_slice(&((inode.i_blocks >> 32) as u16).to_le_bytes());
        out[0x76..0x78].copy_from_slice(&((inode.i_file_acl >> 32) as u16).to_le_bytes());
        out[0x78..0x7A].copy_from_slice(&((inode.i_uid >> 16) as u16).to_le_bytes());
        out[0x7A..0x7C].copy_from_slice(&((inode.i_gid >> 16) as u16).to_le_bytes());
        out[0x7C..0x7E].copy_from_slice(&(inode.i_checksum as u16).to_le_bytes());

        if isize > 128 {
            out[0x80..0x82].copy_from_slice(&inode.i_extra_isize.to_le_bytes());
            out[0x82..0x84].copy_from_slice(&((inode.i_checksum >> 16) as u16).to_le_bytes());
        }

        Ok(())
    }
}
