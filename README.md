![Canicula OS](https://picture.hanbings.com/2024/09/22/f1b8f29c20aba151c2c5e987b2c50ddd.png)

<h1 align="center">⭐ Canicula OS</h1>

## ⭐ Canicula OS

感谢 [xv6-rev7](https://pdos.csail.mit.edu/6.828/2012/xv6/book-rev7.pdf)、[xv6（中文文档）](https://th0ar.gitbooks.io/xv6-chinese/content/)、[rCore](https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html) 和 [os.phil-opp.com](https://os.phil-opp.com/zh-CN/) 这样优秀的教材！

那么旅途从这里开始！

## 🔨 快速构建

### Nix 开发环境

如果已启用 [Nix Flakes](https://nixos.wiki/wiki/Flakes)，可以直接进入包含固定 Rust nightly、QEMU、OVMF 和项目工具的开发环境：

```shell
nix develop
make
```

Rust 工具链版本和构建目标继续由 `rust-toolchain.toml` 统一管理。
当前 Flake 支持 Apple Silicon macOS，以及 x86_64/aarch64 Linux。

如果已安装 [direnv](https://direnv.net/) 和 nix-direnv，项目中的 `.envrc` 会在进入目录时自动激活同一个开发环境。首次使用时执行：

```shell
direnv allow
```

### 手动配置

```shell
git submodule init
git submodule update

# 构建 bootloader 和内核
make
# 使用 qemu 运行
make qemu
```

## 📦 博客 / 文档

> [!WARNING]
> 本人还并不是很熟悉 Rust 语言并且这份文档只是作为学习操作系统的知识的记录，还会存在很多错误的地方，仅供参考。
> 还请多多指教！

> [!NOTE]
> Blog 主要为补充性内容，用于补充文档中的前置知识。
> 数字序号部分是主要的文档，用于描述一个内核中应该有的功能。
> Ext 部分补充 “教学” 内核之外的扩展性内容。

[Blog - Rust：使用 uefi-rs 编写一个 UEFI 应用并加载内核](https://blog.hanbings.io/posts/rust-uefi-bootloader/)

[0 - 基本开发环境](docs/dev-environment.md)

[1 - 引导](docs/bootloader.md)

[2 - 中断与异常处理（WIP）](docs/exceptions-and-interrupts.md)

[3 - 段、分段、分页与页表（WIP）](docs/paging.md)

[4 - 内存管理（WIP）](docs/mm.md)

[5 - 进程调度（WIP）](docs/process.md)

[6 - 文件系统（WIP）](bdocs/fs.md)

[7 - 线程、线程通信（WIP）](docs/thread.md)

[8 - 多核（WIP）](docs/muilt-core.md)

[9 - 外部接口：USB、网卡与显卡（WIP）](docs/extend-interface.md)

[10 - 显存映射与图形化（WIP）](docs/graphics.md)

[Ext - 处理器架构（WIP）](docs/architecture.md)

[Ext - 模块化设计（WIP）](docs/design.md)

[Ext - Ext4 文件系统（WIP）](docs/ext4.md)

[Ext - Hypervisor（WIP）](docs/hypervisor.md)

[Ext - 如何调试](docs/debug.md)

**引用名称说明**

| 手册                                                         | 原始链接                                                     | 文中引用名称 |
| ------------------------------------------------------------ | ------------------------------------------------------------ | ------------ |
| Intel® 64 and IA-32 Architectures Software Developer’s Manual Combined Volumes: 1, 2A, 2B, 2C, 2D, 3A, 3B, 3C, 3D, and 4 (Order Number: 253665-086US December 2024) | https://cdrdv2-public.intel.com/843820/325462-sdm-vol-1-2abcd-3abcd-4-1.pdf | Intel 手册   |
| AMD64 Architecture Programmer’s Manual Volumes 1–5 (Publication No. 40332 Revision 4.08 Date April 2024) | https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/40332.pdf | AMD 手册     |
| Unified Extensible Firmware Interface (UEFI) Specification Release 2.11 (Nov 21, 2024) | https://uefi.org/sites/default/files/resources/UEFI_Spec_Final_2.11.pdf | UEFI Spec    |
