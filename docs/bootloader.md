# 引导

> 引导在英文中为 “boot”，是 bootstrap 的缩写，源自于短语 “Pull oneself up by one's bootstraps”，即“靠自己振作起来”。 -- [维基百科 - 引导程序](https://zh.wikipedia.org/wiki/%E5%95%9F%E5%8B%95%E7%A8%8B%E5%BC%8F)

Linux 有 [GRUB2](https://www.gnu.org/software/grub/) 和 [systemd-boot](https://systemd.io/BOOT/)，Windows 有 [Windows Boot Manager](https://learn.microsoft.com/en-us/windows-hardware/drivers/bringup/boot-and-uefi#understanding-the-windows-boot-manager)，Android 有 [U-Boot](https://docs.u-boot.org/en/latest/android/boot-image.html)。

我们也得写一个引导器才行！

## UEFI

UEFI（Unified Extensible Firmware Interface），统一可扩展固件接口，是一个负责连接硬件和软件之间的接口。

本文是为了编写了一个可以加载内核的引导器，因此将对使用 `uefi-rs`、 `Boot Sevrice` 和 `Runtime Service` 以及一些必要的 `Handle` 和 `Procotol` 进行说明，但不会对于 UEFI 本身进行详细的解析，如果对这一方面可以参考 [UEFI 手册](https://uefi.org/specs/UEFI/2.10/index.html)、罗冰老师的《UEFI 编程实践》和戴正华老师的《UEFI 原理与编程》。

### uefi-rs

> Our mission is to provide **safe** and **performant** wrappers for UEFI interfaces, and allow developers to write idiomatic Rust code. -- [uefi-rs](https://github.com/rust-osdev/uefi-rs)

[EDK2](https://github.com/tianocore/edk2) （EFI Development Kit）是 UEFI 的开发工具包，使用 C 语言进行 UEFI 工程编程。[uefi-rs](https://github.com/rust-osdev/uefi-rs) 是 rust 语言下的 EDK2 封装，巧妙运用了很多 rust 语言的语言特性，使得开发效率大大提升。

现有大多数的 UEFI 编程资料是基于 C 语言的，使用了很多指针特性来实现功能。在 Rust 中我们有更好的写法抽象和隐藏或安全传递这些指针，因此本节主要目的是说明 C 语言的写法与 Rust 写法的异同，以便应对阅读参考资料代码时的语言障碍。如果您有 C / C++ 基础且掌握 Rust 语言那就更好了！

#### 数据类型

从数据类型说起：

在 EDK2 中，为了适配多种不同架构不同位数的 CPU 而对 C 语言的数据类型系统进行了封装，这些数据类型基本能够对应到 Rust 的类型系统中，下表是从 UEFI 手册中抽取的一部分，完整表格在[这里](https://uefi.org/specs/UEFI/2.10/02_Overview.html#data-types)查看。

| EDK2 Type | Rust / uefi-rs Type                                                         | Desciption                                                                                                                                                                                     |
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

#### 阅读 UEFI 手册

一般来说，在 EDK2 中函数的返回值为 EFI_STATUS 类型，（返回的）数据地址会赋值给参数类型为指针的 _OUT_ 参数中，这意味着调用一个函数的步骤是：

1. 在手册中找到函数所在的 `Table`、`Service`、`Handle` 和 `Protocol` 等对应的数据结构，以函数指针 `->` 的方式访问函数。
2. 查看哪些是 _IN_ 类型参数，哪些是 _OUT_ 类型参数
3. 准备好用于 _OUT_ 类型参数的空指针
4. 调用后判断 EFI_STATUS 而得到 _OUT_ 类型参数的指针是否已指向数据
5. 从 _OUT_ 类型参数取出数据

**入口函数**

EDK2：

```c
EFI_STATUS EFIAPI main (
   IN EFI_HANDLE ImageHandle,
   IN EFI_SYSTEM_TABLE *SystemTable
) { }
```

uefi-rs：

```rust
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status { }
```

可以看到 IN 类型数据写法实际上是没有什么区别的，但在 Rust 中能够隐藏指针类型和添加准确的泛型。

在入口中 Image Handle 指向当前 Image（其实也就是当前 EFI 程序），System Table 是一个 UEFI 环境下的全局资源表，存有一些公共数据和函数。

**调用函数**

以获取 Graphics Output Protocol 为例子：

EDK2：

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

而在 uefi-rs 中，基于 Rust 的特性，可以使用 Result 替换掉 EFI_STATUS 这种需要额外声明一个变量来存放状态的方式。

uefi-rs：

```rust
let graphics_output_protocol_handle = boot_table
    .get_handle_for_protocol::<GraphicsOutput>()
    // 返回类型为 Result<Handle>
    // 这里便于理解直接使用了 unwarp，但在正常编码中，应该使用 map_or 或 expect 等方式显式处理错误。
    // 尤其是在 UEFI 这类难于调试的环境下，应该尽可能地留下有用的错误信息
    .unwrap();

let mut graphics_output_protocol = boot_table
    .open_protocol_exclusive::<GraphicsOutput>(graphics_output_protocol_handle)
    // 返回类型为 Result<ScopedProtocol<GraphicsOutputProtocol>>
    .unwrap();
```

### x86-64

### AArch64

这里本来应该还有一份 AArch64 的适配代码，~~但因为有点懒了~~ 稍后再补充。

## RustSBI

SBI（Supervisor Binary Interface）

### RISC-V64
