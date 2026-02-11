#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod error;
pub mod io;
pub mod layout;
pub mod traits;

// The design calls the module `core/`, but that name shadows the `core` crate.
// We use `#[path]` so the directory stays `core/` while the Rust module is `fs_core`.
#[path = "alloc/mod.rs"]
pub mod fs_alloc;
#[path = "core/mod.rs"]
pub mod fs_core;

#[cfg(test)]
mod tests;

// Re-exports
pub use error::Ext4Error;
pub use fs_alloc::bitmap::{
    clear_bit, count_zeros, find_first_zero, find_zero_run, set_bit, test_bit,
};
pub use fs_alloc::block_alloc::{BlockGroupAllocState, Ext4BlockAllocator};
pub use fs_alloc::inode_alloc::{Ext4InodeAllocator, InodeGroupAllocState};
pub use fs_core::block_group_manager::BlockGroupManager;
pub use fs_core::dir_reader::DirReader;
pub use fs_core::extent_walker::{ExtentWalker, PhysicalMapping};
pub use fs_core::file_reader::FileReader;
pub use fs_core::inode_reader::InodeReader;
pub use fs_core::path_resolver::{MAX_SYMLINK_DEPTH, PathResolver};
pub use fs_core::superblock_manager::SuperBlockManager;
pub use fs_core::symlink::SymlinkReader;
pub use io::block_reader::BlockReader;
pub use io::buffer_cache::BufferCache;
pub use layout::block_group::BlockGroupDesc;
pub use layout::checksum::{crc32c, crc32c_raw};
pub use layout::dir_entry::{DirEntry, FileType as DirEntryFileType};
pub use layout::extent::{EXTENT_HEADER_MAGIC, Extent, ExtentHeader, ExtentIndex};
pub use layout::htree::{
    DX_HASH_HALF_MD4, DX_HASH_HALF_MD4_UNSIGNED, DX_HASH_LEGACY, DX_HASH_LEGACY_UNSIGNED,
    DX_HASH_TEA, DX_HASH_TEA_UNSIGNED, DxEntry, DxNode, DxRoot, compute_hash as htree_compute_hash,
};
pub use layout::inode::Inode;
pub use layout::superblock::SuperBlock;
pub use traits::allocator::{BlockAllocator, InodeAllocator};
pub use traits::block_device::BlockDevice;
