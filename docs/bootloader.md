# 引导

> 引导在英文中为 “boot”，是 bootstrap 的缩写，源自于短语 “Pull oneself up by one's bootstraps”，即“靠自己振作起来”。 -- [维基百科 - 引导程序](https://zh.wikipedia.org/wiki/%E5%95%9F%E5%8B%95%E7%A8%8B%E5%BC%8F)

Linux 有 [GRUB2](https://www.gnu.org/software/grub/) 和 [systemd-boot](https://systemd.io/BOOT/)，Windows 有 [Windows Boot Manager](https://learn.microsoft.com/en-us/windows-hardware/drivers/bringup/boot-and-uefi#understanding-the-windows-boot-manager)，Android 有 [U-Boot](https://docs.u-boot.org/en/latest/android/boot-image.html)。

我们也得写一个引导器才行！

## UEFI

UEFI（Unified Extensible Firmware Interface），统一可扩展固件接口，是一个负责连接硬件和软件之间的接口。

本文是为了编写了一个可以加载内核的引导器，因此将对使用 `uefi-rs`、 `Boot Sevrice` 和 `Runtime Service` 以及一些必要的 `Handle` 和 `Procotol` 进行说明，但不会对于 UEFI 本身进行详细的解析，如果对这一方面可以参考 [UEFI 手册](https://uefi.org/specs/UEFI/2.10/index.html)、罗冰老师的《UEFI 编程实践》和戴正华老师的《UEFI 原理与编程》。

### 数据结构

**Boot Service**

**Runtime Service**

**Handle**

**Procotol**

### uefi-rs

> Our mission is to provide **safe** and **performant** wrappers for UEFI interfaces, and allow developers to write idiomatic Rust code. -- [uefi-rs](https://github.com/rust-osdev/uefi-rs)

这个库是 rust 语言下的 UEFI 封装，巧妙运用了很多 rust 语言的语言特性，使得开发效率大大提升。

现有大多数的 UEFI 编程资料是基于 C / C++ 语言的，在 C / C++ 语言中使用了很多指针特性来实现功能。在 rust 中我们有更好的写法传递这些指针，因此本节主要目的是说明 C / C++ 语言的写法与 rust 写法的异同，以便应对阅读参考资料代码时的语言障碍。如果您有 C / C++ 基础且掌握 rust 语言那就更好了！

### x86-64

### AArch64

这里本来应该还有一份 AArch64 的适配代码，~~但因为有点懒了~~ 稍后再补充。

## RustSBI

SBI（Supervisor Binary Interface）

### RISC-V64
