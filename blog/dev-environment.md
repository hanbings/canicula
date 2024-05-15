# 基本开发环境

参考：https://rcore-os.cn/rCore-Tutorial-Book-v3/chapter0/5setup-devel-env.html

但上文提及的 QEMU 有些旧。

这里使用最新的 QEMU。

本文使用的 Linux 发行版是 `debian`，如果使用 `ubuntu` 或 `linux mint` 等衍生版本应该不会在软件安装上出现问题。

## 构建 QEMU

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
   $ ./configure --target-list=riscv64-softmmu,riscv64-linux-user --enable-sdl --enable-slirp
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
   $ rustup target add riscv64gc-unknown-none-elf
   $ cargo install cargo-binutils
   $ rustup component add llvm-tools-preview
   ```

2. 使用如下指令构建内核模块。

   ```shell
   $ cargo build --bin kernel --release
   ```

3. 将 ELF 格式转换为二进制格式。

   ```shell
   $ rust-objcopy \
       --binary-architecture=riscv64 target/riscv64gc-unknown-none-elf/release/kernel \
       --strip-all -O binary target/riscv64gc-unknown-none-elf/release/kernel.bin
   ```

   如果提示 `rust-objcopy` 未找到需要补全工具链

   ```shell
   $ cargo install cargo-binutils
   $ rustup component add llvm-tools-preview
   ```

4. [下载](https://github.com/rustsbi/rustsbi-qemu/releases) 适合 QEMU 使用的 rustsbi 二进制文件。

   解压获得 `rustsbi-qemu.bin` 文件，它将作为 QEMU 的 BIOS 文件，使用 QEMU 启动内核。

   ```shell
   $ qemu-system-riscv64 \
       -machine virt \
       -nographic \
       -bios rustsbi-qemu.bin \
       -device loader,file=target/riscv64gc-unknown-none-elf/release/kernel.bin,addr=0x80200000
   ```
