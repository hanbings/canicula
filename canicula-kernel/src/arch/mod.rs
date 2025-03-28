#[cfg(target_arch = "aarch64")]
#[path = "aarch64/mod.rs"]
pub mod aarch;
#[cfg(target_arch = "riscv64")]
#[path = "riscv64/mod.rs"]
pub mod riscv;
#[cfg(target_arch = "x86_64")]
#[path = "x86/mod.rs"]
pub mod x86;

pub trait Arch {
    fn entry(&mut self) -> !;
}
