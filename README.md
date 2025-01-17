![Canicula OS](https://picture.hanbings.com/2024/09/22/f1b8f29c20aba151c2c5e987b2c50ddd.png)

<h1 align="center">⭐ Canicula OS</h1>

## ⭐ Canicula OS

本分支为 [Rust：使用 uefi-rs 编写一个 UEFI 应用并加载内核](https://blog.hanbings.io/posts/rust-uefi-bootloader) 的示例代码。

### 编译运行

> 注意替换 `$(OVMF_CODE_PATH)` 和 `$(OVMF_VARS_PATH)`
> 

```bash
cargo build --bin canicula-efi --target x86_64-unknown-uefi
mkdir -p esp/efi/boot/
cp target/x86_64-unknown-uefi/debug/canicula-efi.efi esp/efi/boot/bootx64.efi

nasm kernel.asm -o esp/kernel

qemu-system-x86_64 \
    -m 256 \
    -enable-kvm \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE_PATH) \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS_PATH) \
    -drive format=raw,file=fat:rw:esp
```