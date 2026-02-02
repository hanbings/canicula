![Canicula OS](https://picture.hanbings.com/2024/09/22/f1b8f29c20aba151c2c5e987b2c50ddd.png)

<h1 align="center">⭐ Canicula OS</h1>

## ⭐ Canicula OS

### 编译运行

> 注意替换 `$(OVMF_CODE_PATH)` 和 `$(OVMF_VARS_PATH)`
> 

```bash
cargo build -Z build-std=core,alloc,compiler_builtins -p canicula-efi --release --target x86_64-unknown-uefi
cargo build -Z build-std=core,alloc,compiler_builtins -Z json-target-spec -p canicula-kernel --release --target canicula-kernel/x86_64-canicula-kernel.json

cp target/x86_64-canicula-kernel/release/canicula-kernel esp/kernel
cp target/x86_64-unknown-uefi/release/canicula-efi.efi esp/efi/boot/bootx64.efi

echo "Build complete!"

qemu-system-x86_64 \
    -m 256M \
    -display none \
    -serial mon:stdio \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_VARS_4M.fd \
    -drive format=raw,file=fat:rw:esp
```