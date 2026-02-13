ENTRY(kernel_main)

KERNEL_BEGIN = 0xfffff80000000000;

SECTIONS {

  . = KERNEL_BEGIN;

  .rodata ALIGN(4K):
  {
    *(.rodata .rodata.*)
  }

  .text ALIGN(4K):
  {
    *(.text .text.*)
    /* x86 SMP AP trampoline (copied to a low 4KiB page at runtime). */
    KEEP(*(.ap_trampoline))
  }

  .data ALIGN(4K):
  {
    *(.data .data.*)
  }

  .got ALIGN(4K):
  {
    *(.got .got.*)
  }

  .bss ALIGN(4K):
  {
    *(.bss .bss.*)
  }
}
