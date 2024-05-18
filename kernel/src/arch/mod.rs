#[cfg(target_arch = "aarch64")]
pub mod aarch;
#[cfg(target_arch = "riscv64")]
pub mod riscv;
#[cfg(target_arch = "x86_64")]
pub mod x86;
