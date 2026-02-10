mod test {
    use crate::Ext4FS;
    use canicula_common::fs::OperateError;
    use std::fs::{remove_file, File};
    use std::io::Read;
    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    const DEVICE_SIZE: usize = 4096;
    const SUPER_BLOCK_OFFSET: usize = 1024;
    const EXT4_MAGIC: u16 = 0xEF53;

    static DEVICE: Mutex<[u8; DEVICE_SIZE]> = Mutex::new([0; DEVICE_SIZE]);
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn read_byte(offset: usize) -> Result<u8, OperateError> {
        let guard = DEVICE.lock().map_err(|_| OperateError::SystemInterrupted)?;
        guard.get(offset).copied().ok_or(OperateError::Fault)
    }

    fn write_byte(byte: u8, offset: usize) -> Result<usize, OperateError> {
        let mut guard = DEVICE.lock().map_err(|_| OperateError::SystemInterrupted)?;
        let slot = guard.get_mut(offset).ok_or(OperateError::Fault)?;
        *slot = byte;
        Ok(1)
    }

    fn write_u16_le(offset: usize, value: u16) {
        write_byte((value & 0x00FF) as u8, offset).expect("write low byte");
        write_byte((value >> 8) as u8, offset + 1).expect("write high byte");
    }

    fn write_u32_le(offset: usize, value: u32) {
        write_byte((value & 0x0000_00FF) as u8, offset).expect("write byte 0");
        write_byte(((value >> 8) & 0x0000_00FF) as u8, offset + 1).expect("write byte 1");
        write_byte(((value >> 16) & 0x0000_00FF) as u8, offset + 2).expect("write byte 2");
        write_byte(((value >> 24) & 0x0000_00FF) as u8, offset + 3).expect("write byte 3");
    }

    fn clear_device() {
        let mut guard = DEVICE.lock().expect("lock device");
        guard.fill(0);
    }

    fn load_device_prefix_from_file(path: &str) {
        let mut file = File::open(path).expect("open ext4 image");
        let mut prefix = [0u8; DEVICE_SIZE];
        file.read_exact(&mut prefix).expect("read image prefix");
        let mut guard = DEVICE.lock().expect("lock device");
        guard.copy_from_slice(&prefix);
    }

    #[test]
    fn test_probe_reads_ext4_super_block_header() {
        let _test_lock = TEST_LOCK.lock().expect("lock test serial");
        clear_device();

        write_u32_le(SUPER_BLOCK_OFFSET, 1000);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x04, 8000);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x0C, 7000);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x10, 900);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x18, 2);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x20, 32768);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x28, 8192);
        write_u16_le(SUPER_BLOCK_OFFSET + 0x38, EXT4_MAGIC);
        write_u16_le(SUPER_BLOCK_OFFSET + 0x58, 256);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x60, 0x0000_0002);
        write_u32_le(SUPER_BLOCK_OFFSET + 0x64, 0x0000_0001);

        let mut fs: Ext4FS<DEVICE_SIZE> = Ext4FS::new(read_byte, write_byte);
        let header = fs.probe().expect("probe ext4 super block");

        assert_eq!(header.inodes_count, 1000);
        assert_eq!(header.blocks_count_lo, 8000);
        assert_eq!(header.free_blocks_count_lo, 7000);
        assert_eq!(header.free_inodes_count, 900);
        assert_eq!(header.log_block_size, 2);
        assert_eq!(header.block_size(), 4096);
        assert_eq!(header.blocks_per_group, 32768);
        assert_eq!(header.inodes_per_group, 8192);
        assert_eq!(header.magic, EXT4_MAGIC);
        assert_eq!(header.inode_size, 256);
        assert_eq!(header.feature_incompat, 0x0000_0002);
        assert_eq!(header.feature_ro_compat, 0x0000_0001);
        assert!(fs.super_block().is_some());
    }

    #[test]
    fn test_probe_rejects_invalid_magic() {
        let _test_lock = TEST_LOCK.lock().expect("lock test serial");
        clear_device();
        write_u16_le(SUPER_BLOCK_OFFSET + 0x38, 0x1234);

        let mut fs: Ext4FS<DEVICE_SIZE> = Ext4FS::new(read_byte, write_byte);
        let err = fs.probe().expect_err("invalid magic should fail");
        assert!(matches!(err, OperateError::IO));
    }

    #[test]
    fn test_probe_returns_fault_when_super_block_out_of_bounds() {
        let _test_lock = TEST_LOCK.lock().expect("lock test serial");
        let mut fs: Ext4FS<{ SUPER_BLOCK_OFFSET + 2 }> = Ext4FS::new(read_byte, write_byte);
        let err = fs.probe().expect_err("out of bounds should fail");
        assert!(matches!(err, OperateError::Fault));
    }

    #[test]
    fn test_probe_reads_real_ext4_image() {
        let _test_lock = TEST_LOCK.lock().expect("lock test serial");
        if !Path::new("/sbin/mkfs.ext4").exists() {
            eprintln!("skip real ext4 image test: /sbin/mkfs.ext4 not found");
            return;
        }

        clear_device();

        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let image_path = format!("/tmp/canicula-ext4-{uniq}.img");

        let image_file = File::create(&image_path).expect("create ext4 image file");
        image_file
            .set_len(8 * 1024 * 1024)
            .expect("preallocate ext4 image file");

        let mkfs = Command::new("/sbin/mkfs.ext4")
            .arg("-F")
            .arg(&image_path)
            .output()
            .expect("spawn mkfs.ext4");
        assert!(
            mkfs.status.success(),
            "mkfs.ext4 failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&mkfs.stdout),
            String::from_utf8_lossy(&mkfs.stderr)
        );

        load_device_prefix_from_file(&image_path);
        let _ = remove_file(&image_path);

        let mut fs: Ext4FS<DEVICE_SIZE> = Ext4FS::new(read_byte, write_byte);
        let header = fs.probe().expect("probe real ext4 image");
        assert_eq!(header.magic, EXT4_MAGIC);
        assert!(header.block_size() >= 1024);
        assert!(header.inode_size >= 128);
        assert!(header.blocks_per_group > 0);
        assert!(header.inodes_per_group > 0);
    }
}
