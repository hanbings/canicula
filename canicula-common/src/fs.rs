use core::usize;

#[derive(Debug)]
pub enum OperateError {
    InvalidFileDescriptor,
    Fault,
    SystemInterrupted,
    IO,
    DeviceNoFreeSpace,
    NotFoundDev,
    TimeOut,
}

pub trait Fs {
    fn read<const SIZE: usize>(path: &str) -> Result<[u8; SIZE], OperateError>;
    fn write<const SIZE: usize>(path: &str, content: [u8; SIZE]) -> Result<usize, OperateError>;
}
