.att_syntax prefix
.section .ap_trampoline, "ax"
.global ap_trampoline_start
.global ap_trampoline_end

/*
 * x86_64 AP startup trampoline.
 *
 * The SIPI vector points to this page (must be below 1MiB, 4KiB-aligned).
 * The kernel copies this whole section into that page and fills `ap_data`
 * at offset 0x800.
 *
 * The trampoline:
 *   real mode -> protected mode -> long mode -> call Rust AP entry.
 *
 * All addresses are position-dependent within the page (we rely on segment base
 * equal to the page physical base in protected mode; and RIP-relative in long mode).
 */

ap_trampoline_start:
    .code16
    cli
    cld

    /* Use CS as a base for data accesses in real mode. */
    movw %cs, %ax
    movw %ax, %ds

    /* Compute page base physical address: base = cs << 4 */
    xor %eax, %eax
    movw %cs, %ax
    shl $4, %eax

    /* Save base for later patching */
    movl %eax, BASE_TMP_OFF

    /* Patch segment bases in the GDT to match the page base. */
    movl %eax, %ebx
    /* entry 1: CODE32 */
    movw %bx, GDT_OFF + 8*1 + 2
    shr $16, %ebx
    movb %bl, GDT_OFF + 8*1 + 4
    movb $0,  GDT_OFF + 8*1 + 7

    movl BASE_TMP_OFF, %ebx
    /* entry 2: DATA32 */
    movw %bx, GDT_OFF + 8*2 + 2
    shr $16, %ebx
    movb %bl, GDT_OFF + 8*2 + 4
    movb $0,  GDT_OFF + 8*2 + 7

    movl BASE_TMP_OFF, %ebx
    /* entry 3: CODE64 */
    movw %bx, GDT_OFF + 8*3 + 2
    shr $16, %ebx
    movb %bl, GDT_OFF + 8*3 + 4
    movb $0,  GDT_OFF + 8*3 + 7

    movl BASE_TMP_OFF, %ebx
    /* entry 4: DATA64 */
    movw %bx, GDT_OFF + 8*4 + 2
    shr $16, %ebx
    movb %bl, GDT_OFF + 8*4 + 4
    movb $0,  GDT_OFF + 8*4 + 7

    /* Patch GDTR base = page_base + offset(gdt) */
    movl BASE_TMP_OFF, %ebx
    addl $GDT_OFF, %ebx
    movl %ebx, GDT_DESC_OFF + 2

    lgdt GDT_DESC_OFF

    /* Enter protected mode */
    movl %cr0, %eax
    orl $0x1, %eax
    movl %eax, %cr0

    /* Use section-relative offsets to avoid high-VMA relocations. */
    ljmp $0x08, $(pm_entry - ap_trampoline_start)

pm_entry:
    .code32
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    /* Load CR3 (low 32 bits is sufficient under our paging setup) */
    movl AP_DATA_OFF + 0, %eax
    movl %eax, %cr3

    /* Enable PAE */
    movl %cr4, %eax
    orl $(1 << 5), %eax
    movl %eax, %cr4

    /* Enable long mode (EFER.LME) */
    movl $0xC0000080, %ecx
    rdmsr
    orl $(1 << 8), %eax
    wrmsr

    /* Enable paging (CR0.PG) */
    movl %cr0, %eax
    orl $0x80000000, %eax
    movl %eax, %cr0

    /*
     * Far jump to 64-bit code segment to activate long mode.
     *
     * IMPORTANT: in long mode CS base is ignored, so the target RIP must be the
     * absolute (linear) address mapping to the trampoline page. We build an
     * m16:32 far pointer at a fixed offset in this 4KiB page:
     *   offset = page_base + (lm_entry - ap_trampoline_start)
     *   selector = 0x18
     */
    movl BASE_TMP_OFF, %eax
    addl $LM_ENTRY_OFF, %eax
    movl %eax, LM_PTR_OFF
    movw $0x18, LM_PTR_OFF + 4
    ljmp *LM_PTR_OFF

lm_entry:
    .code64
    movw $0x20, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    /* Set stack pointer */
    movq ap_data + 16(%rip), %rsp

    /* Signal BSP that this AP is alive */
    movl $1, ap_data + 32(%rip)

    /* Arguments: (cpu_id, apic_id) */
    movl ap_data + 24(%rip), %edi
    movl ap_data + 28(%rip), %esi

    /* Call Rust entry */
    movq ap_data + 8(%rip), %rax
    call *%rax

1:
    hlt
    jmp 1b

    .align 8
gdt:
    .quad 0x0000000000000000
    /* 0x08: 32-bit code: base patched, limit 0xFFFFF, G=1, D=1 */
    .quad 0x00CF9A000000FFFF
    /* 0x10: 32-bit data: base patched, limit 0xFFFFF, G=1, D=1 */
    .quad 0x00CF92000000FFFF
    /* 0x18: 64-bit code: base patched (ignored in long mode), L=1, G=1 */
    .quad 0x00AF9A000000FFFF
    /* 0x20: 64-bit data: base patched (ignored in long mode) */
    .quad 0x00AF92000000FFFF
gdt_end:

gdt_desc:
    .word (gdt_end - gdt - 1)
    .long 0

base_tmp:
    .long 0

lm_far_ptr:
    .long 0
    .word 0

    /* Put AP data at a fixed offset within the 4KiB page. */
    .org 0x800
ap_data:
    /* +0  */ .long 0          /* cr3_low */
    /* +4  */ .long 0          /* reserved */
    /* +8  */ .quad 0          /* entry (u64) */
    /* +16 */ .quad 0          /* stack_top (u64) */
    /* +24 */ .long 0          /* cpu_id (u32) */
    /* +28 */ .long 0          /* apic_id (u32) */
    /* +32 */ .long 0          /* ack (u32) */
    /* +36 */ .long 0          /* pad */

ap_trampoline_end:

/* Compile-time offsets within the 4KiB trampoline page (no relocations). */
.equ GDT_OFF,      gdt      - ap_trampoline_start
.equ GDT_DESC_OFF, gdt_desc - ap_trampoline_start
.equ BASE_TMP_OFF, base_tmp - ap_trampoline_start
.equ LM_PTR_OFF,   lm_far_ptr - ap_trampoline_start
.equ LM_ENTRY_OFF, lm_entry - ap_trampoline_start
.equ AP_DATA_OFF,  ap_data  - ap_trampoline_start

