![Canicula OS](https://picture.hanbings.com/2024/09/22/f1b8f29c20aba151c2c5e987b2c50ddd.png)

<h1 align="center">⭐ Canicula OS</h1>

## ⭐ Canicula OS

本分支为尝试以 [rust-osdev/bootloader](https://github.com/rust-osdev/bootloader) （使用了 fork 版本 [hanbings/bootloader](https://github.com/hanbings/bootloader) ）作为基本引导器的测试代码。

### 编译运行

```bash
git submodule init
cd bootloader/uefi
cargo build --target x86_64-unknown-uefi --release -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem
cd ../..

make
make qemu
```