#![allow(dead_code)]

use alloc::vec::Vec;

/// CRC32c Castagnoli polynomial in reflected form.
const CRC32C_POLY: u32 = 0x82F63B78;

/// Standard CRC32c (with initial and final complement).
///
/// Matches the well-known CRC32c: `crc32c(0, b"123456789") == 0xE3069283`.
pub fn crc32c(initial: u32, data: &[u8]) -> u32 {
    let mut crc = !initial;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (CRC32C_POLY & mask);
        }
    }
    !crc
}

/// Raw CRC32c without initial/final complement.
///
/// Matches the Linux kernel's `__crc32c_le()` and e2fsprogs' `ext2fs_crc32c_le()`.
/// ext4 metadata checksums use this variant with seed `!0u32`.
pub fn crc32c_raw(seed: u32, data: &[u8]) -> u32 {
    let mut crc = seed;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (CRC32C_POLY & mask);
        }
    }
    crc
}

// Superblock checksum

/// ext4 superblock checksum.
///
/// `crc32c_raw(!0, sb[0..0x3FC])` — CRC over the first 1020 bytes
/// (everything before the checksum field at offset 0x3FC).
pub fn superblock_checksum(sb_bytes: &[u8; 1024]) -> u32 {
    crc32c_raw(!0u32, &sb_bytes[..0x3FC])
}

/// Verify ext4 superblock checksum.
pub fn superblock_checksum_matches(sb_bytes: &[u8; 1024], stored: u32) -> bool {
    superblock_checksum(sb_bytes) == stored
}

// Block group descriptor checksum

/// ext4 block group descriptor checksum (low 16 bits).
///
/// `crc32c_raw(csum_seed, group_no_le || desc_with_checksum_zeroed)`
/// where `csum_seed` is pre-computed from UUID or `s_checksum_seed`.
pub fn block_group_checksum(csum_seed: u32, group_no: u32, desc_bytes: &[u8]) -> u16 {
    let mut desc: Vec<u8> = desc_bytes.into();
    // Zero bg_checksum field at 0x1E..0x20
    if desc.len() >= 0x20 {
        desc[0x1E] = 0;
        desc[0x1F] = 0;
    }

    let crc = crc32c_raw(csum_seed, &group_no.to_le_bytes());
    let crc = crc32c_raw(crc, &desc);
    (crc & 0xFFFF) as u16
}

/// Verify ext4 block group descriptor checksum.
pub fn block_group_checksum_matches(
    csum_seed: u32,
    group_no: u32,
    desc_bytes: &[u8],
    stored: u16,
) -> bool {
    block_group_checksum(csum_seed, group_no, desc_bytes) == stored
}

// Inode checksum

/// ext4 inode checksum.
///
/// `crc32c_raw(inode_seed, inode_with_checksums_zeroed)`
/// where `inode_seed = crc32c_raw(crc32c_raw(csum_seed, ino_le), generation_le)`.
pub fn inode_checksum(csum_seed: u32, ino: u32, generation: u32, inode_bytes: &[u8]) -> u32 {
    let mut inode: Vec<u8> = inode_bytes.into();
    // Zero i_checksum_lo at 0x7C..0x7E
    if inode.len() >= 0x7E {
        inode[0x7C] = 0;
        inode[0x7D] = 0;
    }
    // Zero i_checksum_hi at 0x82..0x84 (if extra inode fields present)
    if inode.len() >= 0x84 {
        inode[0x82] = 0;
        inode[0x83] = 0;
    }

    let seed = crc32c_raw(csum_seed, &ino.to_le_bytes());
    let seed = crc32c_raw(seed, &generation.to_le_bytes());
    crc32c_raw(seed, &inode)
}

/// Verify ext4 inode checksum.
pub fn inode_checksum_matches(
    csum_seed: u32,
    ino: u32,
    generation: u32,
    inode_bytes: &[u8],
    stored: u32,
    full_32bit: bool,
) -> bool {
    let computed = inode_checksum(csum_seed, ino, generation, inode_bytes);
    if full_32bit {
        computed == stored
    } else {
        (computed & 0xFFFF) == (stored & 0xFFFF)
    }
}
