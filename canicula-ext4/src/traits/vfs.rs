use alloc::string::String;
use alloc::vec::Vec;

use crate::error::Result;
use crate::layout::dir_entry::DirEntry;
use crate::layout::inode::Inode;

/// Filesystem-level statistics.
#[derive(Debug, Clone)]
pub struct StatFs {
    pub block_size: u64,
    pub total_blocks: u64,
    pub free_blocks: u64,
    pub total_inodes: u64,
    pub free_inodes: u64,
}

/// High-level filesystem lifecycle operations.
pub trait FileSystem {
    fn unmount(&mut self) -> Result<()>;

    /// Flush dirty data and metadata to disk without unmounting.
    fn sync(&mut self) -> Result<()>;

    /// Return filesystem statistics (block/inode counts, sizes).
    fn stat_fs(&self) -> Result<StatFs>;
}

/// Inode-oriented operations exposed to upper VFS layer.
pub trait InodeOps {
    fn lookup(&self, parent: u32, name: &str) -> Result<u32>;
    fn read(&self, ino: u32, offset: u64, buf: &mut [u8]) -> Result<usize>;
    fn readdir(&self, ino: u32) -> Result<Vec<DirEntry>>;

    fn create(&mut self, parent: u32, name: &str, mode: u16, uid: u32, gid: u32) -> Result<u32>;
    fn write(&mut self, ino: u32, offset: u64, data: &[u8]) -> Result<usize>;
    fn unlink(&mut self, parent: u32, name: &str) -> Result<()>;
    fn mkdir(&mut self, parent: u32, name: &str, mode: u16, uid: u32, gid: u32) -> Result<u32>;
    fn rmdir(&mut self, parent: u32, name: &str) -> Result<()>;
    fn rename(
        &mut self,
        old_parent: u32,
        old_name: &str,
        new_parent: u32,
        new_name: &str,
    ) -> Result<()>;
    fn truncate(&mut self, ino: u32, new_size: u64) -> Result<()>;
    fn symlink(&mut self, parent: u32, name: &str, target: &str, uid: u32, gid: u32)
    -> Result<u32>;

    /// Read the target of a symbolic link.
    fn readlink(&self, ino: u32) -> Result<String>;

    /// Return the inode metadata (stat).
    fn stat(&self, ino: u32) -> Result<Inode>;

    /// Change file mode bits.
    fn chmod(&mut self, ino: u32, mode: u16) -> Result<()>;

    /// Change file owner and group.
    fn chown(&mut self, ino: u32, uid: u32, gid: u32) -> Result<()>;

    /// Update access and modification timestamps.
    fn utimes(&mut self, ino: u32, atime: u32, mtime: u32) -> Result<()>;

    /// Create a hard link.
    fn link(&mut self, parent: u32, name: &str, ino: u32) -> Result<()>;
}
