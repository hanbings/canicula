# 引导

> 引导在英文中为 “boot”，是 bootstrap 的缩写，源自于短语 “Pull oneself up by one's bootstraps”，即“靠自己振作起来”。 -- [维基百科 - 引导程序](https://zh.wikipedia.org/wiki/%E5%95%9F%E5%8B%95%E7%A8%8B%E5%BC%8F)

Linux 有 [GRUB2](https://www.gnu.org/software/grub/) 和 [systemd-boot](https://systemd.io/BOOT/)，Windows 有 [Windows Boot Manager](https://learn.microsoft.com/en-us/windows-hardware/drivers/bringup/boot-and-uefi#understanding-the-windows-boot-manager)，Android 有 [U-Boot](https://docs.u-boot.org/en/latest/android/boot-image.html)。

我们也得写一个引导器才行！

## UEFI

UEFI（Unified Extensible Firmware Interface），统一可扩展固件接口，是一个负责连接硬件和软件之间的接口。

本文是为了编写了一个可以加载内核的引导器，因此将对使用 `uefi-rs`、 `Boot Service` 和 `Runtime Service` 以及一些必要的 `Handle` 和 `Protocol` 进行说明，但不会对于 UEFI 本身进行详细的解析，如果对这一方面可以参考 [UEFI 手册](https://uefi.org/specs/UEFI/2.10/index.html)、罗冰老师的《UEFI 编程实践》和戴正华老师的《UEFI 原理与编程》。

### uefi-rs

> Our mission is to provide **safe** and **performant** wrappers for UEFI interfaces, and allow developers to write idiomatic Rust code. -- [uefi-rs](https://github.com/rust-osdev/uefi-rs)

[EDK2](https://github.com/tianocore/edk2) （EFI Development Kit）是 UEFI 的开发工具包，使用 C 语言进行 UEFI 工程编程。[uefi-rs](https://github.com/rust-osdev/uefi-rs) 是 rust 语言下的 EDK2 封装，巧妙运用了很多 rust 语言的语言特性，使得开发效率大大提升。

现有大多数的 UEFI 编程资料是基于 C 语言的，使用了很多指针特性来实现功能。在 Rust 中我们有更好的写法抽象和隐藏或安全传递这些指针，因此本节主要目的是说明 C 语言的写法与 Rust 写法的异同，以便应对阅读参考资料代码时的语言障碍。如果您有 C / C++ 基础且掌握 Rust 语言那就更好了！

#### 数据类型

从数据类型说起：

在 EDK2 中，为了适配多种不同架构不同位数的 CPU 而对 C 语言的数据类型系统进行了封装，这些数据类型基本能够对应到 Rust 的类型系统中，下表是从 UEFI 手册中抽取的一部分，完整表格在[这里](https://uefi.org/specs/UEFI/2.10/02_Overview.html#data-types)查看。

| EDK2 Type | Rust / uefi-rs Type                                                         | Description                                                                                                                                                                                    |
| --------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| _BOOLEAN_ | bool                                                                        | Logical Boolean. 1-byte value containing a 0 for **FALSE** or a 1 for **TRUE**. Other values are undefined.                                                                                    |
| _INTN_    | iszie                                                                       | Signed value of native width. (4 bytes on supported 32-bit processor instructions, 8 bytes on supported 64-bit processor instructions, 16 bytes on supported 128-bit processor instructions)   |
| _UINTN_   | usize                                                                       | Unsigned value of native width. (4 bytes on supported 32-bit processor instructions, 8 bytes on supported 64-bit processor instructions, 16 bytes on supported 128-bit processor instructions) |
| _INT8_    | i8                                                                          | 1-byte signed value.                                                                                                                                                                           |
| _UINT8_   | u8                                                                          | 1-byte unsigned value.                                                                                                                                                                         |
| _INT16_   | i16                                                                         | 2-byte signed value.                                                                                                                                                                           |
| _UINT16_  | u16                                                                         | 2-byte unsigned value.                                                                                                                                                                         |
| _INT32_   | i32                                                                         | 4-byte signed value.                                                                                                                                                                           |
| _UINT32_  | u32                                                                         | 4-byte unsigned value.                                                                                                                                                                         |
| _INT64_   | i64                                                                         | 8-byte signed value.                                                                                                                                                                           |
| _UINT64_  | u64                                                                         | 8-byte unsigned value.                                                                                                                                                                         |
| _INT128_  | i128                                                                        | 16-byte signed value.                                                                                                                                                                          |
| _UINT128_ | u128                                                                        | 16-byte unsigned value.                                                                                                                                                                        |
| _CHAR8_   | CStr8                                                                       | 1-byte character. Unless otherwise specified, all 1-byte or ASCII characters and strings are stored in 8-bit ASCII encoding format, using the ISO-Latin-1 character set.                       |
| _CHAR16_  | [CStr16](https://docs.rs/uefi/latest/uefi/data_types/struct.CString16.html) | 2-byte Character. Unless otherwise specified all characters and strings are stored in the UCS-2 encoding format as defined by Unicode 2.1 and ISO/IEC 10646 standards.                         |

其中，CStr8 和 CStr16 可以分别使用宏 [cstr8](https://docs.rs/uefi-macros/latest/uefi_macros/macro.cstr8.html) 和 [cstr16](https://docs.rs/uefi-macros/latest/uefi_macros/macro.cstr16.html) 进行构建。

此外常用的还有：

**EFI_STATUS**，用于表达函数返回状态（是否出错，是否有值）。

**EFI_HANDLE**，即是后续我们会提到的 Handle。

#### 修饰符

在 UEFI 手册中的接口描述中，使用了一些助记词作为参数的修饰符，如下：

| **Mnemonic** | **Description**                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------- |
| _IN_         | Datum is passed to the function.                                                                        |
| _OUT_        | Datum is returned from the function.                                                                    |
| _OPTIONAL_   | Passing the datum to the function is optional, and a _NULL_ may be passed if the value is not supplied. |
| _CONST_      | Datum is read-only.                                                                                     |
| _EFIAPI_     | Defines the calling convention for UEFI interfaces.                                                     |

#### 入口函数

**EDK2：**

```c
EFI_STATUS EFIAPI main (
   IN EFI_HANDLE ImageHandle,
   IN EFI_SYSTEM_TABLE *SystemTable
) { }
```

**uefi-rs：**

```rust
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status { }
```

可以看到 IN 类型数据写法实际上是没有什么区别的，但在 Rust 中能够隐藏指针类型和添加准确的泛型。

在入口中 Image Handle 指向当前 Image（其实也就是当前 EFI 程序），System Table 是一个 UEFI 环境下的全局资源表，存有一些公共数据和函数。

#### 调用函数

一般来说，在 EDK2 中函数的返回值为 EFI*STATUS 类型，（返回的）数据地址会赋值给参数类型为指针的 \_OUT* 参数中，这意味着调用一个函数的步骤是：

1. 在手册中找到函数所在的 `Table`、`Service`、`Handle` 和 `Protocol` 等对应的数据结构，以函数指针 `->` 的方式访问函数。
2. 查看哪些是 _IN_ 类型参数，哪些是 _OUT_ 类型参数
3. 准备好用于 _OUT_ 类型参数的空指针
4. 调用后判断 EFI*STATUS 而得到 \_OUT* 类型参数的指针是否已指向数据
5. 从 _OUT_ 类型参数取出数据

以获取 Graphics Output Protocol 为例子：

**EDK2：**

使用 [LocateProtocol](https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html?highlight=locateprotocol#efi-boot-services-locateprotocol) 函数获取 Graphics Output Protocol。

其函数原型为：

```c
typedef
EFI_STATUS
(EFIAPI *EFI_LOCATE_PROTOCOL) (
  IN EFI_GUID                            *Protocol,
  IN VOID                                *Registration OPTIONAL,
  OUT VOID                               **Interface
 );
```

我们需要关注的是第三个参数 Interface，可以看到是一个指针类型的 OUT 类型参数。

> On return, a pointer to the first interface that matches _Protocol_ and _Registration_. -- EFI_LOCATE_PROTOCOL - Interface

因此有代码：

```c
// 声明一个状态，用于接受函数表明执行状态的返回值
EFI_STATUS Status;
// 提前声明一个指针用于指向函数的返回值数据
EFI_GRAPHICS_OUTPUT_PROTOCOL *GraphicsOutput;

// gBS 是 BootService，通过 SystemTable->BootService 获取
Status = gBS->LocateProtocol(
    // gEfiGraphicsOutputProtocolGuid 定义在头文件中，是 Graphics Output Protocol 的 UUID
    &gEfiGraphicsOutputProtocolGuid,
    NULL,
    (VOID **)&GraphicsOutput
);
if (EFI_ERROR(Status)) {
    return Status;
}
```

**uefi-rs：**

基于 Rust 的特性，可以使用 Result 替换掉 EFI_STATUS 这种需要额外声明一个变量来存放状态的方式。

```rust
let graphics_output_protocol_handle = boot_service
    .get_handle_for_protocol::<GraphicsOutput>()
    // 返回类型为 Result<Handle>
    // 这里便于理解直接使用了 unwarp，但在正常编码中，应该使用 map_or 或 expect 等方式显式处理错误。
    // 尤其是在 UEFI 这类难于调试的环境下，应该尽可能地留下有用的错误信息
    .unwrap();

let mut graphics_output_protocol = boot_service
    .open_protocol_exclusive::<GraphicsOutput>(graphics_output_protocol_handle)
    // 返回类型为 Result<ScopedProtocol<GraphicsOutputProtocol>>
    .unwrap();
```

### x86-64

要加载内核，一共有三步！

**第一步**：~~把冰箱门打开~~ 初始化 Boot Service 和加载 Protocol

```rust
// 为了 println 宏能够正常使用，还需要先初始化工具类
uefi::helpers::init(&mut system_table).unwrap();

// 加载系统服务
let boot_services = system_table.boot_services();

// 加载 Simple File System Handle
let simple_file_system_handle = boot_services
    .get_handle_for_protocol::<SimpleFileSystem>()
    .expect("Cannot get protocol handle");

// 从 Handle 中获取 Protocol
let mut simple_file_system_protocol = boot_services
    .open_protocol_exclusive::<SimpleFileSystem>(simple_file_system_handle)
    .expect("Cannot get simple file system protocol");
```

**第二步**：开辟内存空间，先将内核路径名字加载到内存，再将内核文件信息加载到内存，最后再把内核文件本体加载到内存

```rust
pub const FILE_BUFFER_SIZE: usize = 0x400;
pub const PAGE_SIZE: usize = 0x1000;
pub const KERNEL_PATH: &str = "\\canicula-kernel";

// 我们的内核名称为 canicula-kernel，目录在 esp/canicula-kernel
// 即是 UEFI 在 QEMU 能读取到的标卷的根目录
// 所以我们只需要获取一个根目录就够了
let mut root = simple_file_system_protocol
    .open_volume()
    .expect("Cannot open volume");

// 先创建一个路径名称的缓冲区（实际上并不需要这么大的空间 我们的路径没有这么长）
let mut kernel_path_buffer = [0u16; FILE_BUFFER_SIZE];
// 将路径转为 CStr16 类型
let kernel_path = CStr16::from_str_with_buf(KERNEL_PATH, &mut kernel_path_buffer)
    .expect("Invalid kernel path!");
// 然后在根目录下以文件名形式获取 File Handle
let kernel_file_handle = root
    .open(kernel_path, FileMode::Read, FileAttribute::empty())
    .expect("Cannot open kernel file");
// 但注意只是获取到了文件的 Handle，文件还没有真正加载到内存
let mut kernel_file = match kernel_file_handle.into_type().unwrap() {
    FileType::Regular(f) => f,
    _ => panic!("This file does not exist!"),
};

// 为了将文件真正加载到内存还需要文件的长度（也就是大小）
// 这个长度在文件信息里
// 所以为文件信息开辟一片缓冲区，然后将它读取到这里
let mut kernel_file_info_buffer = [0u8; FILE_BUFFER_SIZE];
let kernel_file_info: &mut FileInfo = kernel_file
    .get_info(&mut kernel_file_info_buffer)
    .expect("Cannot get file info");
// 然后拿到文件长度
let kernel_file_size =
    usize::try_from(kernel_file_info.file_size()).expect("Invalid file size!");

// 接着要用 allocate_pages 开辟一篇内存空间，确保内核可以独自占用一片内存空间
let kernel_file_address = boot_services
    .allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        // 先用内核长度除以页大小，然后再额外多加一页
        // 这样就能保证开辟的内存能装得下内核了
        // 页是 UEFI 内存管理机制的一部分，可以搜索关键词 “内存管理页表” 了解这部分的内容，本文不再详细展开了
        kernel_file_size / PAGE_SIZE + 1,
    )
    .expect("Cannot allocate memory in the RAM!") as *mut u8;

// 防止这块地址以前有其他程序写入过内容
// 我们用 0 再填充一次
let kernel_file_in_memory = unsafe {
    core::ptr::write_bytes(kernel_file_address, 0, kernel_file_size);
    core::slice::from_raw_parts_mut(kernel_file_address, kernel_file_size)
};
// 最后用 Handle 的 read 函数将内核文件内容转写到这块内存中
// 这个 kernel_file_size 指的是读进内存的长度
let kernel_file_size = kernel_file
    .read(kernel_file_in_memory)
    .expect("Cannot read file into the memory!");
```

### AArch64

这里本来应该还有一份 AArch64 的适配代码，~~但因为有点懒了~~ 稍后再补充。

## RustSBI

SBI（Supervisor Binary Interface）

### RISC-V64
