#![cfg_attr(not(test), no_std)]

use canicula_common::fs::OperateError;
use types::super_block::{EXT4_SUPER_BLOCK_MAGIC, SuperBlock, SuperBlockHeader};

#[cfg(test)]
mod tests;
mod types;

#[allow(unused)]
pub struct Ext4FS<const SIZE: usize> {
    read_byte: fn(usize) -> Result<u8, OperateError>,
    write_byte: fn(u8, usize) -> Result<usize, OperateError>,
    super_block: Option<SuperBlockHeader>,
}

#[allow(unused)]
impl<const SIZE: usize> Ext4FS<SIZE> {
    pub fn new(
        read_byte: fn(usize) -> Result<u8, OperateError>,
        write_byte: fn(u8, usize) -> Result<usize, OperateError>,
    ) -> Self {
        Ext4FS {
            read_byte,
            write_byte,
            super_block: None,
        }
    }

    fn read_byte_at(&self, offset: usize) -> Result<u8, OperateError> {
        if offset >= SIZE {
            return Err(OperateError::Fault);
        }
        (self.read_byte)(offset)
    }

    fn read_u16_le_at(&self, offset: usize) -> Result<u16, OperateError> {
        let b0 = self.read_byte_at(offset)? as u16;
        let b1 = self.read_byte_at(offset + 1)? as u16;
        Ok(b0 | (b1 << 8))
    }

    fn read_u32_le_at(&self, offset: usize) -> Result<u32, OperateError> {
        let b0 = self.read_byte_at(offset)? as u32;
        let b1 = self.read_byte_at(offset + 1)? as u32;
        let b2 = self.read_byte_at(offset + 2)? as u32;
        let b3 = self.read_byte_at(offset + 3)? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    fn read_u16_field(&self, field: SuperBlock) -> Result<u16, OperateError> {
        self.read_u16_le_at(field.absolute_offset())
    }

    fn read_u32_field(&self, field: SuperBlock) -> Result<u32, OperateError> {
        self.read_u32_le_at(field.absolute_offset())
    }

    pub fn read_super_block_header(&self) -> Result<SuperBlockHeader, OperateError> {
        let inodes_count = self.read_u32_field(SuperBlock::InodesCount)?;
        let blocks_count_lo = self.read_u32_field(SuperBlock::BlocksCountLo)?;
        let free_blocks_count_lo = self.read_u32_field(SuperBlock::FreeBlocksCountLo)?;
        let free_inodes_count = self.read_u32_field(SuperBlock::FreeInodesCount)?;
        let log_block_size = self.read_u32_field(SuperBlock::LogBlockSize)?;
        let blocks_per_group = self.read_u32_field(SuperBlock::BlocksPerGroup)?;
        let inodes_per_group = self.read_u32_field(SuperBlock::InodesPerGroup)?;
        let magic = self.read_u16_field(SuperBlock::Magic)?;
        let inode_size = self.read_u16_field(SuperBlock::InodeSize)?;
        let feature_incompat = self.read_u32_field(SuperBlock::FeatureIncompat)?;
        let feature_ro_compat = self.read_u32_field(SuperBlock::FeatureRoCompat)?;

        Ok(SuperBlockHeader {
            inodes_count,
            blocks_count_lo,
            free_blocks_count_lo,
            free_inodes_count,
            log_block_size,
            blocks_per_group,
            inodes_per_group,
            magic,
            inode_size,
            feature_incompat,
            feature_ro_compat,
        })
    }

    pub fn probe(&mut self) -> Result<&SuperBlockHeader, OperateError> {
        let header = self.read_super_block_header()?;
        if header.magic != EXT4_SUPER_BLOCK_MAGIC {
            return Err(OperateError::IO);
        }
        self.super_block = Some(header);
        Ok(self.super_block.as_ref().expect("header just inserted"))
    }

    pub fn super_block(&self) -> Option<&SuperBlockHeader> {
        self.super_block.as_ref()
    }
}
