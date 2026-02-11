use alloc::vec::Vec;

use crate::error::Result;
use crate::layout::dir_entry::DirEntry;

/// High-level filesystem lifecycle operations.
pub trait FileSystem {
    fn unmount(&mut self) -> Result<()>;
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
}
