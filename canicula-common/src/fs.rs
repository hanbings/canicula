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
