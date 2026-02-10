mod test {
    use crate::SuperBlock;
    use crate::error::Ext4Error;
    use crate::fs_core::superblock_manager::SuperBlockManager;
    use crate::io::block_reader::BlockReader;
    use crate::layout::superblock::{
        EXT4_SUPER_MAGIC, INCOMPAT_64BIT, INCOMPAT_EXTENTS, INCOMPAT_FILETYPE, INCOMPAT_FLEX_BG,
        SUPER_BLOCK_SIZE,
    };
    use crate::traits::block_device::BlockDevice;

    // MemoryBlockDevice

    /// In-memory block device for testing.
    struct MemoryBlockDevice {
        data: Vec<u8>,
        block_size: usize,
    }

    impl MemoryBlockDevice {
        fn new(size: usize, block_size: usize) -> Self {
            Self {
                data: vec![0u8; size],
                block_size,
            }
        }

        /// Write a little-endian u16 at the given **byte** offset (raw data access).
        fn write_u16_le(&mut self, offset: usize, value: u16) {
            let bytes = value.to_le_bytes();
            self.data[offset..offset + 2].copy_from_slice(&bytes);
        }

        /// Write a little-endian u32 at the given **byte** offset (raw data access).
        fn write_u32_le(&mut self, offset: usize, value: u32) {
            let bytes = value.to_le_bytes();
            self.data[offset..offset + 4].copy_from_slice(&bytes);
        }
    }

    impl BlockDevice for MemoryBlockDevice {
        fn read_block(&self, block_no: u64, buf: &mut [u8]) -> Result<(), Ext4Error> {
            let offset = block_no as usize * self.block_size;
            if offset + buf.len() > self.data.len() {
                return Err(Ext4Error::OutOfBounds);
            }
            buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
            Ok(())
        }

        fn write_block(&mut self, block_no: u64, buf: &[u8]) -> Result<(), Ext4Error> {
            let offset = block_no as usize * self.block_size;
            if offset + buf.len() > self.data.len() {
                return Err(Ext4Error::OutOfBounds);
            }
            self.data[offset..offset + buf.len()].copy_from_slice(buf);
            Ok(())
        }

        fn block_size(&self) -> usize {
            self.block_size
        }

        fn total_blocks(&self) -> u64 {
            (self.data.len() / self.block_size) as u64
        }

        fn flush(&mut self) -> Result<(), Ext4Error> {
            Ok(())
        }
    }

    // Helpers

    const SUPER_BLOCK_OFF: usize = 1024;

    /// Create a memory device with a minimal valid ext4 super block.
    fn make_minimal_superblock_device() -> MemoryBlockDevice {
        let mut dev = MemoryBlockDevice::new(8192, 1024);

        // Basic counts
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x00, 1000); // s_inodes_count
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x04, 8000); // s_blocks_count_lo
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x0C, 7000); // s_free_blocks_count_lo
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x10, 900); // s_free_inodes_count

        // Geometry
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x14, 0); // s_first_data_block (0 for block_size >= 2048)
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x18, 2); // s_log_block_size → block_size = 4096
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x20, 32768); // s_blocks_per_group
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x28, 8192); // s_inodes_per_group

        // Magic
        dev.write_u16_le(SUPER_BLOCK_OFF + 0x38, 0xEF53);

        // Inode size & desc size
        dev.write_u16_le(SUPER_BLOCK_OFF + 0x58, 256); // s_inode_size
        dev.write_u16_le(SUPER_BLOCK_OFF + 0xFE, 32); // s_desc_size

        // Features: FILETYPE only (minimal)
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x60, INCOMPAT_FILETYPE); // s_feature_incompat
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x64, 0x0001); // s_feature_ro_compat (sparse_super)

        // Journal inode
        dev.write_u32_le(SUPER_BLOCK_OFF + 0xE0, 8); // s_journal_inum (default)

        dev
    }

    // Tests

    #[test]
    fn test_parse_superblock_fields() {
        let dev = make_minimal_superblock_device();
        let reader = BlockReader::new(dev);

        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        assert_eq!(super_block.s_inodes_count, 1000);
        assert_eq!(super_block.s_blocks_count_lo, 8000);
        assert_eq!(super_block.s_free_blocks_count_lo, 7000);
        assert_eq!(super_block.s_free_inodes_count, 900);
        assert_eq!(super_block.s_first_data_block, 0);
        assert_eq!(super_block.s_log_block_size, 2);
        assert_eq!(super_block.block_size(), 4096);
        assert_eq!(super_block.s_blocks_per_group, 32768);
        assert_eq!(super_block.s_inodes_per_group, 8192);
        assert_eq!(super_block.s_magic, EXT4_SUPER_MAGIC);
        assert_eq!(super_block.s_inode_size, 256);
        assert_eq!(super_block.s_desc_size, 32);
        assert_eq!(super_block.s_feature_incompat, INCOMPAT_FILETYPE);
        assert_eq!(super_block.s_journal_inum, 8);
    }

    #[test]
    fn test_superblock_validate_ok() {
        let dev = make_minimal_superblock_device();
        let reader = BlockReader::new(dev);

        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();
        super_block.validate().unwrap();
    }

    #[test]
    fn test_superblock_convenience_methods() {
        let dev = make_minimal_superblock_device();
        let reader = BlockReader::new(dev);

        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        // block_count = 8000 (no 64bit, so hi ignored)
        assert_eq!(super_block.block_count(), 8000);
        assert_eq!(super_block.free_blocks_count(), 7000);
        // group_count = ceil(8000 / 32768) = 1
        assert_eq!(super_block.group_count(), 1);

        assert!(!super_block.has_64bit());
        assert!(!super_block.has_extents());
        assert!(!super_block.has_metadata_csum());
        assert!(!super_block.has_flex_bg());
    }

    #[test]
    fn test_superblock_64bit_block_count() {
        let mut dev = make_minimal_superblock_device();
        // Enable 64-bit feature
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x60, INCOMPAT_FILETYPE | INCOMPAT_64BIT);
        // Set blocks_count_hi
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x150, 1); // hi = 1

        let reader = BlockReader::new(dev);
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        assert!(super_block.has_64bit());
        // block_count = (1 << 32) | 8000 = 4294975296
        assert_eq!(super_block.block_count(), (1u64 << 32) | 8000);
    }

    #[test]
    fn test_superblock_feature_flags() {
        let mut dev = make_minimal_superblock_device();
        dev.write_u32_le(
            SUPER_BLOCK_OFF + 0x60,
            INCOMPAT_FILETYPE | INCOMPAT_EXTENTS | INCOMPAT_64BIT | INCOMPAT_FLEX_BG,
        );

        let reader = BlockReader::new(dev);
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        assert!(super_block.has_extents());
        assert!(super_block.has_64bit());
        assert!(super_block.has_flex_bg());
    }

    #[test]
    fn test_super_block_manager_load() {
        let dev = make_minimal_superblock_device();
        let reader = BlockReader::new(dev);
        let mgr = SuperBlockManager::load(&reader).unwrap();

        assert_eq!(mgr.block_size, 4096);
        assert_eq!(mgr.group_count, 1);
        assert!(!mgr.is_64bit);
        assert!(!mgr.has_metadata_csum);
        assert_eq!(mgr.desc_size, 32);
        assert_eq!(mgr.super_block.s_inodes_count, 1000);
    }

    #[test]
    fn test_super_block_manager_64bit_desc_size() {
        let mut dev = make_minimal_superblock_device();
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x60, INCOMPAT_FILETYPE | INCOMPAT_64BIT);
        dev.write_u16_le(SUPER_BLOCK_OFF + 0xFE, 64); // desc_size = 64

        let reader = BlockReader::new(dev);
        let mgr = SuperBlockManager::load(&reader).unwrap();

        assert!(mgr.is_64bit);
        assert_eq!(mgr.desc_size, 64);
    }

    #[test]
    fn test_super_block_manager_64bit_desc_size_clamped() {
        let mut dev = make_minimal_superblock_device();
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x60, INCOMPAT_FILETYPE | INCOMPAT_64BIT);
        dev.write_u16_le(SUPER_BLOCK_OFF + 0xFE, 32); // desc_size = 32 but 64bit → clamp to 64

        let reader = BlockReader::new(dev);
        let mgr = SuperBlockManager::load(&reader).unwrap();

        assert!(mgr.is_64bit);
        assert_eq!(mgr.desc_size, 64); // clamped
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let mut dev = MemoryBlockDevice::new(8192, 1024);
        dev.write_u16_le(SUPER_BLOCK_OFF + 0x38, 0x1234); // wrong magic

        let reader = BlockReader::new(dev);
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();

        let result = SuperBlock::parse(&raw);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Ext4Error::InvalidMagic));
    }

    #[test]
    fn test_super_block_manager_rejects_invalid_magic() {
        let mut dev = MemoryBlockDevice::new(8192, 1024);
        dev.write_u16_le(SUPER_BLOCK_OFF + 0x38, 0x1234);

        let reader = BlockReader::new(dev);
        let result = SuperBlockManager::load(&reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_rejects_bad_log_block_size() {
        let mut dev = make_minimal_superblock_device();
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x18, 7); // log_block_size = 7 → invalid

        let reader = BlockReader::new(dev);
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        let result = super_block.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Ext4Error::CorruptedFs(_)));
    }

    #[test]
    fn test_validate_rejects_zero_inodes_per_group() {
        let mut dev = make_minimal_superblock_device();
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x28, 0); // inodes_per_group = 0

        let reader = BlockReader::new(dev);
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        let result = super_block.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_check_features_rejects_unknown_incompat() {
        let mut dev = make_minimal_superblock_device();
        dev.write_u32_le(SUPER_BLOCK_OFF + 0x60, 0x8000_0000); // unknown incompat bit

        let reader = BlockReader::new(dev);
        let mut raw = [0u8; SUPER_BLOCK_SIZE];
        reader.read_bytes(SUPER_BLOCK_OFF as u64, &mut raw).unwrap();
        let super_block = SuperBlock::parse(&raw).unwrap();

        let result = super_block.check_features(false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Ext4Error::IncompatibleFeature(_)
        ));
    }

    #[test]
    fn test_read_bytes_out_of_bounds() {
        let dev = MemoryBlockDevice::new(2048, 1024);
        let reader = BlockReader::new(dev);

        // Try to read past the end of the device
        let mut buf = [0u8; 1024];
        let result = reader.read_bytes(2048, &mut buf);
        assert!(result.is_err());
    }

    // Real ext4 image test

    #[test]
    fn test_real_ext4_image() {
        use std::fs::{File, remove_file};
        use std::io::Read;
        use std::path::Path;
        use std::process::Command;
        use std::time::{SystemTime, UNIX_EPOCH};

        if !Path::new("/sbin/mkfs.ext4").exists() {
            eprintln!("skip: /sbin/mkfs.ext4 not found");
            return;
        }

        // Create a temp ext4 image
        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let image_path = format!("/tmp/canicula-ext4-{uniq}.img");

        let image_file = File::create(&image_path).unwrap();
        image_file.set_len(8 * 1024 * 1024).unwrap();

        let mkfs = Command::new("/sbin/mkfs.ext4")
            .arg("-F")
            .arg(&image_path)
            .output()
            .unwrap();
        assert!(
            mkfs.status.success(),
            "mkfs.ext4 failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&mkfs.stdout),
            String::from_utf8_lossy(&mkfs.stderr),
        );

        // Read the full image into memory
        let mut file = File::open(&image_path).unwrap();
        let mut image_data = Vec::new();
        file.read_to_end(&mut image_data).unwrap();
        let _ = remove_file(&image_path);

        // Load via the new architecture
        let mut dev = MemoryBlockDevice::new(image_data.len(), 1024);
        dev.data = image_data;

        let reader = BlockReader::new(dev);
        let mgr = SuperBlockManager::load(&reader).unwrap();

        // Verify fundamental invariants
        assert_eq!(mgr.super_block.s_magic, EXT4_SUPER_MAGIC);
        assert!(mgr.block_size >= 1024);
        assert!(mgr.super_block.s_inode_size >= 128);
        assert!(mgr.super_block.s_blocks_per_group > 0);
        assert!(mgr.super_block.s_inodes_per_group > 0);
        assert!(mgr.group_count > 0);

        // Print filesystem info (visible with `cargo test -- --nocapture`)
        eprintln!("--- Real ext4 image parameters ---");
        eprintln!("  block_size:       {}", mgr.block_size);
        eprintln!("  group_count:      {}", mgr.group_count);
        eprintln!("  is_64bit:         {}", mgr.is_64bit);
        eprintln!("  has_metadata_csum:{}", mgr.has_metadata_csum);
        eprintln!("  desc_size:        {}", mgr.desc_size);
        eprintln!("  block_count:      {}", mgr.super_block.block_count());
        eprintln!("  free_blocks:      {}", mgr.super_block.free_blocks_count());
        eprintln!("  has_extents:      {}", mgr.super_block.has_extents());
        eprintln!("  has_flex_bg:      {}", mgr.super_block.has_flex_bg());
        eprintln!("  has_dir_index:    {}", mgr.super_block.has_dir_index());
        eprintln!("  incompat:         0x{:08X}", mgr.super_block.s_feature_incompat);
        eprintln!("  ro_compat:        0x{:08X}", mgr.super_block.s_feature_ro_compat);
    }
}
