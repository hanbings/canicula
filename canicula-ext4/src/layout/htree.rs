#![allow(dead_code)]

use alloc::vec::Vec;

use crate::error::{Ext4Error, Result};
use crate::layout::read_u32_le;

// Hash version constants

pub const DX_HASH_LEGACY: u8 = 0;
pub const DX_HASH_HALF_MD4: u8 = 1;
pub const DX_HASH_TEA: u8 = 2;
pub const DX_HASH_LEGACY_UNSIGNED: u8 = 3;
pub const DX_HASH_HALF_MD4_UNSIGNED: u8 = 4;
pub const DX_HASH_TEA_UNSIGNED: u8 = 5;

// DxEntry

/// A single HTree index entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DxEntry {
    pub hash: u32,
    pub block: u32, // logical block number within directory file
}

// DxRoot

/// Parsed HTree root header (logical block 0 of an indexed directory).
#[derive(Debug, Clone)]
pub struct DxRoot {
    pub hash_version: u8,
    pub indirection_levels: u8,
    pub limit: u16,
    pub count: u16,
    /// entries[0] is the catch-all (hash=0), entries[1..] are sorted hash/block pairs.
    pub entries: Vec<DxEntry>,
}

impl DxRoot {
    /// Parse dx_root from directory logical block 0.
    ///
    /// Layout (all offsets from block start):
    /// - 0x00..0x18: fake "." + ".." directory entries (24 bytes)
    /// - 0x18..0x20: dx_root_info (8 bytes) — includes hash_version, indirection_levels
    /// - 0x20..0x24: dx_countlimit (limit, count) — overlaid on entries[0].hash
    /// - 0x24..0x28: entries[0].block (catch-all block)
    /// - 0x28..:     entries[1..count-1] as (hash, block) pairs, 8 bytes each
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 40 {
            return Err(Ext4Error::CorruptedFs("dx root block too small"));
        }

        let hash_version = raw[0x1C];
        let indirection_levels = raw[0x1E];
        let limit = u16::from_le_bytes([raw[0x20], raw[0x21]]);
        let count = u16::from_le_bytes([raw[0x22], raw[0x23]]);

        if count == 0 || count > limit {
            return Err(Ext4Error::CorruptedFs("invalid dx root count/limit"));
        }

        let mut entries = Vec::with_capacity(count as usize);

        // entries[0]: catch-all entry (hash meaningless, block at 0x24)
        entries.push(DxEntry {
            hash: 0,
            block: read_u32_le(raw, 0x24),
        });

        // entries[1..count-1]: real hash/block pairs
        let mut off = 0x28usize;
        for _ in 1..count {
            if off + 8 > raw.len() {
                return Err(Ext4Error::CorruptedFs("dx root entries out of bounds"));
            }
            entries.push(DxEntry {
                hash: read_u32_le(raw, off),
                block: read_u32_le(raw, off + 4),
            });
            off += 8;
        }

        Ok(Self {
            hash_version,
            indirection_levels,
            limit,
            count,
            entries,
        })
    }

    /// Choose target logical block for a filename hash.
    ///
    /// entries[0].block is the catch-all (for hashes below entries[1].hash).
    /// Returns the block from the last entry whose hash <= target.
    pub fn lookup_block(&self, hash: u32) -> u32 {
        lookup_in_entries(&self.entries, hash)
    }
}

// DxNode

/// Parsed HTree intermediate node (dx_node).
///
/// Layout:
/// - 0x00..0x08: fake directory entry (8 bytes)
/// - 0x08..0x0C: dx_countlimit (limit, count)
/// - 0x0C..0x10: entries[0].block (catch-all)
/// - 0x10..:     entries[1..count-1] as (hash, block) pairs
#[derive(Debug, Clone)]
pub struct DxNode {
    pub limit: u16,
    pub count: u16,
    pub entries: Vec<DxEntry>,
}

impl DxNode {
    /// Parse a dx_node block.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() < 16 {
            return Err(Ext4Error::CorruptedFs("dx node block too small"));
        }

        let limit = u16::from_le_bytes([raw[0x08], raw[0x09]]);
        let count = u16::from_le_bytes([raw[0x0A], raw[0x0B]]);

        if count == 0 || count > limit {
            return Err(Ext4Error::CorruptedFs("invalid dx node count/limit"));
        }

        let mut entries = Vec::with_capacity(count as usize);

        // entries[0]: catch-all
        entries.push(DxEntry {
            hash: 0,
            block: read_u32_le(raw, 0x0C),
        });

        // entries[1..count-1]
        let mut off = 0x10usize;
        for _ in 1..count {
            if off + 8 > raw.len() {
                return Err(Ext4Error::CorruptedFs("dx node entries out of bounds"));
            }
            entries.push(DxEntry {
                hash: read_u32_le(raw, off),
                block: read_u32_le(raw, off + 4),
            });
            off += 8;
        }

        Ok(Self {
            limit,
            count,
            entries,
        })
    }

    /// Choose target logical block for a filename hash.
    pub fn lookup_block(&self, hash: u32) -> u32 {
        lookup_in_entries(&self.entries, hash)
    }
}

// Shared entry lookup

/// Find the block for `hash` in a sorted entry list.
///
/// entries[0] is the catch-all (hash=0). entries[1..] are sorted by hash.
/// Returns the block of the last entry whose hash <= target.
fn lookup_in_entries(entries: &[DxEntry], hash: u32) -> u32 {
    let mut chosen = entries[0].block;
    for e in &entries[1..] {
        if e.hash <= hash {
            chosen = e.block;
        } else {
            break;
        }
    }
    chosen
}

/// Collect all candidate leaf blocks for a hash (handles hash collisions).
///
/// Returns the primary target block plus any adjacent blocks with the same hash.
pub fn find_candidate_blocks(entries: &[DxEntry], hash: u32) -> Vec<u32> {
    // Find the target entry index
    let mut target_idx = 0usize;
    for (i, e) in entries.iter().enumerate().skip(1) {
        if e.hash <= hash {
            target_idx = i;
        } else {
            break;
        }
    }

    if target_idx == 0 {
        // Catch-all: only this one block
        return alloc::vec![entries[0].block];
    }

    let target_hash = entries[target_idx].hash;
    let mut blocks = Vec::new();

    // Collect ALL entries with the same hash (they may be split across blocks)
    for e in &entries[1..] {
        if e.hash == target_hash {
            blocks.push(e.block);
        } else if e.hash > target_hash {
            break;
        }
    }

    if blocks.is_empty() {
        blocks.push(entries[target_idx].block);
    }

    blocks
}

// ext4 directory hash

/// Compute ext4-compatible directory hash for HTree lookup.
///
/// Implements the hash algorithms from Linux `fs/ext4/hash.c`:
/// - Legacy (versions 0, 3)
/// - Half-MD4 (versions 1, 4)
/// - TEA (versions 2, 5)
///
/// Returns the major hash value (bit 0 cleared, EOF-safe).
pub fn compute_hash(name: &[u8], hash_version: u8, seed: &[u8; 16]) -> u32 {
    if name.is_empty() {
        return 0;
    }

    // Initialize buffer with MD4 initial values
    let mut buf: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];

    // Override with seed if any word is non-zero
    let seed_words: [u32; 4] = [
        u32::from_le_bytes([seed[0], seed[1], seed[2], seed[3]]),
        u32::from_le_bytes([seed[4], seed[5], seed[6], seed[7]]),
        u32::from_le_bytes([seed[8], seed[9], seed[10], seed[11]]),
        u32::from_le_bytes([seed[12], seed[13], seed[14], seed[15]]),
    ];
    if seed_words.iter().any(|&w| w != 0) {
        buf = seed_words;
    }

    let signed = matches!(
        hash_version,
        DX_HASH_LEGACY | DX_HASH_HALF_MD4 | DX_HASH_TEA
    );

    let hash = match hash_version {
        DX_HASH_LEGACY | DX_HASH_LEGACY_UNSIGNED => {
            if signed {
                dx_hack_hash_signed(name)
            } else {
                dx_hack_hash_unsigned(name)
            }
        }
        DX_HASH_HALF_MD4 | DX_HASH_HALF_MD4_UNSIGNED => {
            let mut input = [0u32; 8];
            let mut pos = 0usize;
            while pos < name.len() {
                str2hashbuf(&name[pos..], name.len() - pos, &mut input, 8, signed);
                half_md4_transform(&mut buf, &input);
                pos += 32;
            }
            buf[1]
        }
        DX_HASH_TEA | DX_HASH_TEA_UNSIGNED => {
            let mut input = [0u32; 8];
            let mut pos = 0usize;
            while pos < name.len() {
                str2hashbuf(&name[pos..], name.len() - pos, &mut input, 4, signed);
                tea_transform(&mut buf, &input);
                pos += 16;
            }
            buf[0]
        }
        _ => 0,
    };

    finalize_hash(hash)
}

/// Finalize hash: clear bit 0, avoid EOF marker collision.
fn finalize_hash(hash: u32) -> u32 {
    let h = hash & !1u32;
    // EXT4_HTREE_EOF_32BIT = 0x7FFFFFFF; avoid (EOF << 1) = 0xFFFFFFFE
    if h == 0xFFFFFFFE { 0xFFFFFFFC } else { h }
}

// str2hashbuf

/// Convert a name chunk into a u32 buffer for the hash transform.
///
/// Port of Linux `str2hashbuf_signed` / `str2hashbuf_unsigned`.
/// `msg` is the remaining name bytes from the current position.
/// `remaining_len` is the total remaining length (used for padding).
/// `num` is the number of u32 slots to fill (8 for half-MD4, 4 for TEA).
fn str2hashbuf(msg: &[u8], remaining_len: usize, buf: &mut [u32; 8], num: usize, signed: bool) {
    let pad = (remaining_len as u32) | ((remaining_len as u32) << 8);
    let pad = pad | (pad << 16);

    let mut val = pad;
    let effective_len = remaining_len.min(num * 4).min(msg.len());

    let mut buf_idx = 0usize;
    let mut slots_remaining = num;

    for i in 0..effective_len {
        let byte_val = if signed {
            // Sign-extend: treat as i8 → i32 → u32 (matches C's `(int)(signed char)`)
            msg[i] as i8 as i32 as u32
        } else {
            msg[i] as u32
        };
        val = byte_val.wrapping_add(val << 8);
        if i % 4 == 3 {
            buf[buf_idx] = val;
            buf_idx += 1;
            val = pad;
            slots_remaining -= 1;
        }
    }

    // Fill remaining slots: first with partial val, then with pad
    if slots_remaining > 0 {
        slots_remaining -= 1;
        buf[buf_idx] = val;
        buf_idx += 1;
    }
    while slots_remaining > 0 {
        slots_remaining -= 1;
        buf[buf_idx] = pad;
        buf_idx += 1;
    }
}

// Legacy hash

/// Legacy signed hash (DX_HASH_LEGACY).
fn dx_hack_hash_signed(name: &[u8]) -> u32 {
    let mut hash0: u32 = 0x12a3fe2d;
    let mut hash1: u32 = 0x37abe8f9;

    for &b in name {
        let sb = b as i8 as i32 as u32;
        let hash = hash1.wrapping_add(hash0 ^ sb.wrapping_mul(7152373));
        let hash = if hash & 0x80000000 != 0 {
            hash.wrapping_sub(0x7fffffff)
        } else {
            hash
        };
        hash1 = hash0;
        hash0 = hash;
    }

    hash0 << 1
}

/// Legacy unsigned hash (DX_HASH_LEGACY_UNSIGNED).
fn dx_hack_hash_unsigned(name: &[u8]) -> u32 {
    let mut hash0: u32 = 0x12a3fe2d;
    let mut hash1: u32 = 0x37abe8f9;

    for &b in name {
        let hash = hash1.wrapping_add(hash0 ^ (b as u32).wrapping_mul(7152373));
        let hash = if hash & 0x80000000 != 0 {
            hash.wrapping_sub(0x7fffffff)
        } else {
            hash
        };
        hash1 = hash0;
        hash0 = hash;
    }

    hash0 << 1
}

// Half-MD4

/// Half-MD4 transform (port of Linux `halfMD4Transform`).
fn half_md4_transform(buf: &mut [u32; 4], input: &[u32; 8]) {
    let (mut a, mut b, mut c, mut d) = (buf[0], buf[1], buf[2], buf[3]);

    const K1: u32 = 0;
    const K2: u32 = 0x5A827999; // sqrt(2) * 2^30
    const K3: u32 = 0x6ED9EBA1; // sqrt(3) * 2^30

    // F(x,y,z) = z ^ (x & (y ^ z))
    macro_rules! round_f {
        ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr) => {
            $a = $a.wrapping_add(($d ^ ($b & ($c ^ $d))).wrapping_add($x));
            $a = $a.rotate_left($s);
        };
    }

    // G(x,y,z) = (x & y) + ((x ^ y) & z)
    macro_rules! round_g {
        ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr) => {
            $a = $a.wrapping_add((($b & $c).wrapping_add(($b ^ $c) & $d)).wrapping_add($x));
            $a = $a.rotate_left($s);
        };
    }

    // H(x,y,z) = x ^ y ^ z
    macro_rules! round_h {
        ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr) => {
            $a = $a.wrapping_add(($b ^ $c ^ $d).wrapping_add($x));
            $a = $a.rotate_left($s);
        };
    }

    // Round 1
    round_f!(a, b, c, d, input[0].wrapping_add(K1), 3);
    round_f!(d, a, b, c, input[1].wrapping_add(K1), 7);
    round_f!(c, d, a, b, input[2].wrapping_add(K1), 11);
    round_f!(b, c, d, a, input[3].wrapping_add(K1), 19);
    round_f!(a, b, c, d, input[4].wrapping_add(K1), 3);
    round_f!(d, a, b, c, input[5].wrapping_add(K1), 7);
    round_f!(c, d, a, b, input[6].wrapping_add(K1), 11);
    round_f!(b, c, d, a, input[7].wrapping_add(K1), 19);

    // Round 2
    round_g!(a, b, c, d, input[1].wrapping_add(K2), 3);
    round_g!(d, a, b, c, input[3].wrapping_add(K2), 7);
    round_g!(c, d, a, b, input[5].wrapping_add(K2), 11);
    round_g!(b, c, d, a, input[7].wrapping_add(K2), 19);
    round_g!(a, b, c, d, input[0].wrapping_add(K2), 3);
    round_g!(d, a, b, c, input[2].wrapping_add(K2), 7);
    round_g!(c, d, a, b, input[4].wrapping_add(K2), 11);
    round_g!(b, c, d, a, input[6].wrapping_add(K2), 19);

    // Round 3
    round_h!(a, b, c, d, input[3].wrapping_add(K3), 3);
    round_h!(d, a, b, c, input[7].wrapping_add(K3), 7);
    round_h!(c, d, a, b, input[2].wrapping_add(K3), 11);
    round_h!(b, c, d, a, input[6].wrapping_add(K3), 19);
    round_h!(a, b, c, d, input[1].wrapping_add(K3), 3);
    round_h!(d, a, b, c, input[5].wrapping_add(K3), 7);
    round_h!(c, d, a, b, input[0].wrapping_add(K3), 11);
    round_h!(b, c, d, a, input[4].wrapping_add(K3), 19);

    buf[0] = buf[0].wrapping_add(a);
    buf[1] = buf[1].wrapping_add(b);
    buf[2] = buf[2].wrapping_add(c);
    buf[3] = buf[3].wrapping_add(d);
}

// TEA

const TEA_DELTA: u32 = 0x9E3779B9;

/// TEA transform (port of Linux `TEA_transform`).
///
/// Uses input[0..4] as key; input[4..8] are ignored.
fn tea_transform(buf: &mut [u32; 4], input: &[u32; 8]) {
    let (a, b, c, d) = (input[0], input[1], input[2], input[3]);

    // First pair: buf[0], buf[1]
    let mut sum = 0u32;
    let (mut b0, mut b1) = (buf[0], buf[1]);
    for _ in 0..16 {
        sum = sum.wrapping_add(TEA_DELTA);
        b0 = b0.wrapping_add(
            ((b1 << 4).wrapping_add(a)) ^ (b1.wrapping_add(sum)) ^ ((b1 >> 5).wrapping_add(b)),
        );
        b1 = b1.wrapping_add(
            ((b0 << 4).wrapping_add(c)) ^ (b0.wrapping_add(sum)) ^ ((b0 >> 5).wrapping_add(d)),
        );
    }
    buf[0] = buf[0].wrapping_add(b0);
    buf[1] = buf[1].wrapping_add(b1);

    // Second pair: buf[2], buf[3]
    sum = 0;
    b0 = buf[2];
    b1 = buf[3];
    for _ in 0..16 {
        sum = sum.wrapping_add(TEA_DELTA);
        b0 = b0.wrapping_add(
            ((b1 << 4).wrapping_add(a)) ^ (b1.wrapping_add(sum)) ^ ((b1 >> 5).wrapping_add(b)),
        );
        b1 = b1.wrapping_add(
            ((b0 << 4).wrapping_add(c)) ^ (b0.wrapping_add(sum)) ^ ((b0 >> 5).wrapping_add(d)),
        );
    }
    buf[2] = buf[2].wrapping_add(b0);
    buf[3] = buf[3].wrapping_add(b1);
}
