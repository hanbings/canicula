#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod error;
pub mod io;
pub mod layout;
pub mod traits;

// The design calls the module `core/`, but that name shadows the `core` crate.
// We use `#[path]` so the directory stays `core/` while the Rust module is `fs_core`.
#[path = "core/mod.rs"]
pub mod fs_core;

#[cfg(test)]
mod tests;

// Re-exports
pub use error::Ext4Error;
pub use fs_core::block_group_manager::BlockGroupManager;
pub use fs_core::dir_reader::DirReader;
pub use fs_core::extent_walker::{ExtentWalker, PhysicalMapping};
pub use fs_core::file_reader::FileReader;
pub use fs_core::inode_reader::InodeReader;
pub use fs_core::superblock_manager::SuperBlockManager;
pub use io::block_reader::BlockReader;
pub use io::buffer_cache::BufferCache;
pub use layout::block_group::BlockGroupDesc;
pub use layout::dir_entry::{DirEntry, FileType as DirEntryFileType};
pub use layout::extent::{EXTENT_HEADER_MAGIC, Extent, ExtentHeader, ExtentIndex};
pub use layout::inode::Inode;
pub use layout::superblock::SuperBlock;
pub use traits::block_device::BlockDevice;
