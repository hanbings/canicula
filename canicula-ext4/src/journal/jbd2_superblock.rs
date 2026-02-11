#![allow(dead_code)]

use crate::error::{Ext4Error, Result};

pub const JBD2_MAGIC_NUMBER: u32 = 0xC03B3998;

pub const JBD2_BLOCKTYPE_DESCRIPTOR: u32 = 1;
pub const JBD2_BLOCKTYPE_COMMIT: u32 = 2;
pub const JBD2_BLOCKTYPE_SUPERBLOCK_V1: u32 = 3;
pub const JBD2_BLOCKTYPE_SUPERBLOCK_V2: u32 = 4;
pub const JBD2_BLOCKTYPE_REVOKE: u32 = 5;

#[derive(Debug, Clone, Copy)]
pub struct JournalHeader {
    pub h_magic: u32,
    pub h_blocktype: u32,
    pub h_sequence: u32,
}

#[derive(Debug, Clone)]
pub struct JournalSuperBlock {
    pub header: JournalHeader,
    pub s_blocksize: u32,
    pub s_maxlen: u32,
    pub s_first: u32,
    pub s_sequence: u32,
    pub s_start: u32,
    pub s_errno: u32,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    pub s_nr_users: u32,
    pub s_checksum_type: u8,
    pub s_checksum: u32,
}

impl JournalHeader {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 12 {
            return Err(Ext4Error::CorruptedFs("journal header too small"));
        }
        Ok(Self {
            h_magic: read_u32_be(raw, 0),
            h_blocktype: read_u32_be(raw, 4),
            h_sequence: read_u32_be(raw, 8),
        })
    }
}

impl JournalSuperBlock {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 0xFC + 4 {
            return Err(Ext4Error::CorruptedFs("journal superblock too small"));
        }

        let header = JournalHeader::parse(raw)?;
        if header.h_magic != JBD2_MAGIC_NUMBER {
            return Err(Ext4Error::InvalidMagic);
        }
        if header.h_blocktype != JBD2_BLOCKTYPE_SUPERBLOCK_V1
            && header.h_blocktype != JBD2_BLOCKTYPE_SUPERBLOCK_V2
        {
            return Err(Ext4Error::CorruptedFs("invalid journal superblock type"));
        }

        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&raw[0x30..0x40]);
        Ok(Self {
            header,
            s_blocksize: read_u32_be(raw, 0x0C),
            s_maxlen: read_u32_be(raw, 0x10),
            s_first: read_u32_be(raw, 0x14),
            s_sequence: read_u32_be(raw, 0x18),
            s_start: read_u32_be(raw, 0x1C),
            s_errno: read_u32_be(raw, 0x24),
            s_feature_compat: read_u32_be(raw, 0x28),
            s_feature_incompat: read_u32_be(raw, 0x2C),
            s_feature_ro_compat: read_u32_be(raw, 0x30),
            s_uuid: uuid,
            s_nr_users: read_u32_be(raw, 0x40),
            s_checksum_type: raw[0x50],
            s_checksum: read_u32_be(raw, 0xFC),
        })
    }

    pub fn is_clean(&self) -> bool {
        self.s_start == 0
    }
}

#[inline]
fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
