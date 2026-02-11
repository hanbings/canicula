use crate::error::Result;

/// Journal abstraction used by metadata write paths.
pub trait Journal {
    type Handle;

    fn start_transaction(&mut self) -> Result<Self::Handle>;
    fn get_write_access(&mut self, handle: &Self::Handle, block_no: u64) -> Result<()>;
    fn dirty_metadata(&mut self, handle: &Self::Handle, block_no: u64) -> Result<()>;
    fn commit(&mut self, handle: Self::Handle) -> Result<()>;
    fn abort(&mut self, handle: Self::Handle) -> Result<()>;
}

/// Journal recovery abstraction for mount-time replay.
pub trait JournalRecovery {
    fn needs_recovery(&self) -> bool;
    fn recover(&mut self) -> Result<()>;
}
