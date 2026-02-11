#![allow(dead_code)]

/// Unified error type for canicula-ext4.
#[derive(Debug)]
pub enum Ext4Error {
    /// I/O error from the block device
    IoError,
    /// Corrupted filesystem metadata
    CorruptedFs(&'static str),
    /// Unsupported incompatible feature
    IncompatibleFeature(u32),
    /// Invalid magic number
    InvalidMagic,
    /// Checksum mismatch
    InvalidChecksum,
    /// Access out of device bounds
    OutOfBounds,
    /// Read-only filesystem
    ReadOnly,
    /// File or directory not found
    NotFound,
}

/// Convenience Result type alias.
pub type Result<T> = ::core::result::Result<T, Ext4Error>;
