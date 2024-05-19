<h1 align="center">⭐ Canicula OS</h1>

## ⭐ Canicula OS

感谢 [xv6-rev7](https://pdos.csail.mit.edu/6.828/2012/xv6/book-rev7.pdf)、[xv6（中文文档）](https://th0ar.gitbooks.io/xv6-chinese/content/) 和 [rCore](https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html) 这样优秀的教材！

那么旅途从这里开始！

## 🔨 快速构建

```shell
# 默认构建 x86 架构内核
$ cargo build
# 构建 AArch 64 架构内核
$ cargo build --target aarch64-unknown-none
# 构建 RISC-V 架构内核
$ cargo build --target riscv64gc-unknown-none-elf
# 构建 x86 EFI 文件
$ cargo build --bin canicula_efi --target x86_64-unknown-uefi
# 构建 AArch 64 EFI 文件
$ cargo build --bin canicula_efi --target aarch-unknown-uefi
```

## 📦 博客

[0 - 基本开发环境](docs/dev-environment.md)

[1 - ELF 文件格式（WIP）](docs/efi.md)

[2 - 内存管理（WIP）](docs/mm.md)

[3 - 进程调度（WIP）](docs/process.md)

[4 - CaFS 文件系统（WIP）](bdocs/fs.md)

[5 - 线程、线程通信（WIP）](docs/thread.md)

[6 - 多核（WIP）](docs/muilt-core.md)

[7 - 外部接口：USB、网卡与显卡（WIP）](docs/extend-interface.md)

[8 - 显存映射与图形化（WIP）](docs/graphics.md)
