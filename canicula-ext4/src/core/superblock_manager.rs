use crate::error::Result;
use crate::io::block_reader::BlockReader;
use crate::layout::superblock::{SUPER_BLOCK_OFFSET, SUPER_BLOCK_SIZE, SuperBlock};
use crate::traits::block_device::BlockDevice;

/// Super block manager.
///
/// Loads the super block from disk, validates it, and caches the commonly
/// used derived parameters so that every caller does not need to re-derive them.
///
/// This is the very first step of `mount()`.
pub struct SuperBlockManager {
    /// The parsed super block.
    pub super_block: SuperBlock,
    /// Filesystem block size in bytes (`1024 << s_log_block_size`).
    pub block_size: usize,
    /// Number of block groups.
    pub group_count: u32,
    /// Whether the 64-bit feature is enabled.
    pub is_64bit: bool,
    /// Whether metadata checksumming is enabled.
    pub has_metadata_csum: bool,
    /// Block group descriptor size (64 if 64-bit, else 32).
    pub desc_size: u16,
}

impl SuperBlockManager {
    /// Load the super block from the device via the given block reader.
    ///
    /// 1. Read 1024 raw bytes from byte offset 1024.
    /// 2. `SuperBlock::parse()`.
    /// 3. `validate()` + `check_features(writable=false)`.
    /// 4. Cache derived parameters.
    pub fn load<D: BlockDevice>(reader: &BlockReader<D>) -> Result<Self> {
        // Read 1024 raw bytes starting at byte offset 1024 (the super block).
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFFSET as u64, &mut raw)?;

        // Parse
        let super_block = SuperBlock::parse(&raw)?;

        // Validate structure & features (read-only for now)
        super_block.validate()?;
        super_block.check_features(false)?;

        // Derive cached parameters
        let is_64bit = super_block.has_64bit();
        let has_metadata_csum = super_block.has_metadata_csum();
        let block_size = super_block.block_size();
        let group_count = super_block.group_count();

        // desc_size: 64-bit mode requires at least 64 bytes per descriptor;
        // non-64-bit mode always uses 32 bytes.
        let desc_size = if is_64bit {
            if super_block.s_desc_size >= 64 {
                super_block.s_desc_size
            } else {
                64
            }
        } else {
            32
        };

        Ok(SuperBlockManager {
            super_block,
            block_size,
            group_count,
            is_64bit,
            has_metadata_csum,
            desc_size,
        })
    }
}
