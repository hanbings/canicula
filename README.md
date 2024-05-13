<h1 align="center">⭐ Canicula OS</h1>

## ⭐ Canicula OS（RISC-V）

感谢 [xv6-rev7](https://pdos.csail.mit.edu/6.828/2012/xv6/book-rev7.pdf)、[xv6（中文文档）](https://th0ar.gitbooks.io/xv6-chinese/content/)、[rCore](https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html) 和 [2024S](https://learningos.cn/rCore-Tutorial-Guide-2024S) 这样优秀的教材！

那么旅途从这里开始！

## 📦 构建

1. 安装 Rust 工具链。

   ```shell
   $ rustup default nightly
   $ rustup target add riscv64gc-unknown-none-elf
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

4. [下载](https://github.com/rustsbi/rustsbi-qemu/releases) 适合 QEMU 使用的 rustsbi 二进制文件。

   解压获得 `rustsbi-qemu.bin` 文件，它将作为 QEMU 的 BIOS 文件，使用 QEMU 启动内核。

   ```shell
   $ qemu-system-riscv64 \
       -machine virt \
       -nographic \
       -bios rustsbi-qemu.bin \
       -device loader,file=target/riscv64gc-unknown-none-elf/release/kernel.bin,addr=0x80200000
   ```
