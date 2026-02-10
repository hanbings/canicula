#![cfg_attr(not(test), no_std)]

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
pub use fs_core::superblock_manager::SuperBlockManager;
pub use io::block_reader::BlockReader;
pub use layout::superblock::SuperBlock;
pub use traits::block_device::BlockDevice;
