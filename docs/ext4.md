# 目录
- [目录](#目录)
- [Ext4](#ext4)
  - [构建](#构建)


# Ext4

参考：[Ext4](https://ext4.wiki.kernel.org/index.php/Main_Page)

## 构建

```shell
# 对不同架构进行编译
$ cd canicula-ext4
$ cargo build --target x86_64-unknown-none
$ cargo build --target aarch64-unknown-none
$ cargo build --target riscv64gc-unknown-none-elf
# 运行测试
$ cargo test --target x86_64-unknown-linux-gnu -Z build-std=std -- --show-output
```
