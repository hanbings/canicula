# 基本开发环境

为了支持 x86-64、AArch64 和 RISC-V，我们需要这些架构对应的 QEMU 模拟器和在 Rust 中添加差异处理的代码。

## 构建 QEMU

首先需要构建 [QEMU](https://www.qemu.org/)。

本文基于 Debian 发行版，如果使用其他发行版可能需要自行补齐依赖。

1. 安装编译 QEMU 的构建依赖

   ```shell
   $ sudo apt install autoconf automake autotools-dev curl wget libmpc-dev libmpfr-dev libgmp-dev \
       gawk build-essential bison flex texinfo gperf libtool patchutils bc \
       zlib1g-dev libexpat-dev pkg-config  libglib2.0-dev libpixman-1-dev libsdl2-dev libslirp-dev \
       git tmux python3 python3-pip ninja-build
   ```

2. 下载最新版本的 QEMU 源码

   ```shell
   $ wget https://download.qemu.org/qemu-9.0.0.tar.xz
   $ tar xvJf qemu-9.0.0.tar.xz
   $ cd qemu-9.0.0
   ```

3. 编译

   ```shell
   # 建议是把 --enable-sdl 图形接口支持和 --enable-slirp 网卡支持打开
   $ ./configure --target-list=x86_64-softmmu,x86_64-linux-user, \
       riscv64-softmmu,riscv64-linux-user, \
       aarch64-softmmu,aarch64-linux-user  \
       --enable-sdl --enable-slirp
   $ make -j$(nproc)
   ```

4. 配置环境变量

   ```shell
   # 记得替换 {path} 为你的路径
   $ export PATH=$PATH:/{path}/qemu-9.0.0/build
   ```

## 配置项目环境

1. 安装 Rust 工具链

   ```shell
   $ rustup default nightly
   # UEFI 的构建目标
   $ rustup target add x86_64-unknown-uefi
   $ rustup target add aarch64-unknown-uefi
   # 内核目标架构的构建目标
   $ rustup target add x86_64-unknown-none
   $ rustup target add aarch64-unknown-none
   $ rustup target add riscv64gc-unknown-none-elf
   $ cargo install cargo-binutils
   $ rustup component add llvm-tools-preview
   ```

2. 使用如下指令构建 x86 版本的内核模块。

   ```shell
   $ cargo build
   ```

### x86-64

1. 首先编译 x86-64 的 EFI 文件。

   ```shell
   $ cargo build --bin canicula-efi --target x86_64-unknown-uefi
   $ cp target/x86_64-unknown-uefi/debug/canicula-efi.efi esp/efi/boot/bootx64.efi
   ```

2. 启动 QEMU 虚拟机：

   ```shell
   $ sudo apt install ovmf
   $ qemu-system-x86_64 -enable-kvm \
       -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
       -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_VARS.fd \
       -drive format=raw,file=fat:rw:esp
   ```

如果出现 `qemu-system-x86_64: failed to initialize kvm: Permission denied` 问题可以尝试 `sudo chmod 666 /dev/kvm`。

### AArch64

### RISC-V

在 RISC-V 架构中，我们使用 RustSBI 直接加载内核文件。

1. 将 ELF 格式转换为二进制格式。

   ```shell
   $ rust-objcopy \
       --binary-architecture=riscv64 target/riscv64gc-unknown-none-elf/release/kernel \
       --strip-all -O binary target/riscv64gc-unknown-none-elf/release/kernel.bin
   ```

2. [下载](https://github.com/rustsbi/rustsbi-qemu/releases) 适合 QEMU 使用的 rustsbi 二进制文件。

   解压获得 `rustsbi-qemu.bin` 文件，它将作为 QEMU 的 BIOS 文件，使用 QEMU 启动内核。

   ```shell
   $ qemu-system-riscv64 \
       -machine virt \
       -nographic \
       -bios rustsbi-qemu.bin \
       -device loader,file=target/riscv64gc-unknown-none-elf/release/kernel.bin,addr=0x80200000
   ```

## 如何实现的多架构内核？

算上 UEFI 环境，一共五个环境，三种架构。

项目在 x86-64 和 AArch64 中使用 UEFI，在 RISC-V64 中使用 RustSBI。

## 构建时加入环境变量

使用 `build.rs` 编译脚本即可在编译时惨入（ `println!("cargo::rustc-env={}={}", key, value);`） 环境变量。
