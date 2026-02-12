#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use core::alloc::Layout;
use core::arch::asm;
use core::ptr::NonNull;

use log::{info, warn};
use x86_64::VirtAddr;

use crate::arch::x86::gdt;
use crate::arch::x86::memory;
use crate::arch::x86::qemu;
use crate::arch::x86::virtualization::vmcb;
use crate::arch::x86::virtualization::vmcb::Vmcb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvmError {
    NotSupported,
    DisabledByBios,
    PhysicalAddressUnavailable,
    AllocationFailed,
}

pub struct SvmContext {
    pub hsave: Page4K,
    pub iopm: [Page4K; 3],
    pub msrpm: [Page4K; 2],
    pub vmcb: Box<Vmcb>,
    pub vmcb_pa: u64,
    pub npt_root: Page4K,
}

pub struct Page4K {
    ptr: NonNull<u8>,
    pa: u64,
}

impl Page4K {
    pub fn pa(&self) -> u64 {
        self.pa
    }

    pub fn va(&self) -> VirtAddr {
        VirtAddr::from_ptr(self.ptr.as_ptr())
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for Page4K {
    fn drop(&mut self) {
        unsafe {
            alloc::alloc::dealloc(
                self.ptr.as_ptr(),
                Layout::from_size_align_unchecked(4096, 4096),
            );
        }
    }
}

fn alloc_page4k_zeroed() -> Result<Page4K, SvmError> {
    let layout = Layout::from_size_align(4096, 4096).map_err(|_| SvmError::AllocationFailed)?;
    let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let ptr = NonNull::new(ptr).ok_or(SvmError::AllocationFailed)?;

    let va = VirtAddr::from_ptr(ptr.as_ptr());
    let pa = unsafe { memory::virtual_to_physical_current(va) }
        .ok_or(SvmError::PhysicalAddressUnavailable)?
        .as_u64();

    Ok(Page4K { ptr, pa })
}

#[inline]
fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    #[cfg(target_arch = "x86_64")]
    {
        let r = core::arch::x86_64::__cpuid_count(leaf, subleaf);
        (r.eax, r.ebx, r.ecx, r.edx)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (leaf, subleaf);
        (0, 0, 0, 0)
    }
}

#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((high as u64) << 32) | (low as u64)
}

#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
}

const MSR_EFER: u32 = 0xC000_0080;
const MSR_VM_CR: u32 = 0xC001_0114;
const MSR_VM_HSAVE_PA: u32 = 0xC001_0117;

const EFER_SVME: u64 = 1 << 12;
const VM_CR_SVMDIS: u64 = 1 << 4;

pub fn is_supported() -> bool {
    let (_eax, _ebx, ecx, _edx) = cpuid(0x8000_0001, 0);
    (ecx & (1 << 2)) != 0
}

pub fn init_minimal() -> Result<SvmContext, SvmError> {
    if !is_supported() {
        return Err(SvmError::NotSupported);
    }

    let vm_cr = unsafe { rdmsr(MSR_VM_CR) };
    if (vm_cr & VM_CR_SVMDIS) != 0 {
        return Err(SvmError::DisabledByBios);
    }

    let hsave = alloc_page4k_zeroed()?;
    unsafe { wrmsr(MSR_VM_HSAVE_PA, hsave.pa()) };

    let iopm = [
        alloc_page4k_zeroed()?,
        alloc_page4k_zeroed()?,
        alloc_page4k_zeroed()?,
    ];
    let msrpm = [alloc_page4k_zeroed()?, alloc_page4k_zeroed()?];

    let mut efer = unsafe { rdmsr(MSR_EFER) };
    if (efer & EFER_SVME) == 0 {
        efer |= EFER_SVME;
        unsafe { wrmsr(MSR_EFER, efer) };
    }

    let vmcb = Box::new(Vmcb::zeroed());
    let vmcb_pa = {
        let va = VirtAddr::from_ptr(vmcb.as_ref().as_ptr());
        unsafe { memory::virtual_to_physical_current(va) }
            .ok_or(SvmError::PhysicalAddressUnavailable)?
            .as_u64()
    };

    let npt_root = alloc_page4k_zeroed()?;

    info!(
        "SVM enabled: EFER={:#x}, HSAVE_PA={:#x}, IOPM_PA={:#x}, MSRPM_PA={:#x}, VMCB_PA={:#x}, NPT_ROOT_PA={:#x}",
        unsafe { rdmsr(MSR_EFER) },
        hsave.pa(),
        iopm[0].pa(),
        msrpm[0].pa(),
        vmcb_pa,
        npt_root.pa()
    );

    Ok(SvmContext {
        hsave,
        iopm,
        msrpm,
        vmcb,
        vmcb_pa,
        npt_root,
    })
}

pub fn maybe_init_at_boot() {
    match init_minimal() {
        Ok(ctx) => {
            core::mem::forget(ctx);
            warn!("SVM minimal context initialized (guest run loop not wired yet)");
        }
        Err(e) => {
            warn!("SVM init skipped: {:?}", e);
        }
    }
}

#[repr(C, packed)]
struct DtPtr {
    limit: u16,
    base: u64,
}

#[inline]
unsafe fn sgdt() -> DtPtr {
    let mut dt = DtPtr { limit: 0, base: 0 };
    unsafe {
        asm!("sgdt [{}]", in(reg) &mut dt, options(nostack, preserves_flags));
    }
    dt
}

#[inline]
unsafe fn sidt() -> DtPtr {
    let mut dt = DtPtr { limit: 0, base: 0 };
    unsafe {
        asm!("sidt [{}]", in(reg) &mut dt, options(nostack, preserves_flags));
    }
    dt
}

#[inline]
unsafe fn read_cr0() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr0", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

#[inline]
unsafe fn read_cr4() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr4", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

#[inline]
unsafe fn read_rflags() -> u64 {
    let v: u64;
    unsafe { asm!("pushfq; pop {}", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

pub fn run_test_guest() -> ! {
    const GUEST_STUB: &[u8] = &[0x0f, 0x01, 0xd9, 0xf4];

    let mut ctx = match init_minimal() {
        Ok(ctx) => ctx,
        Err(e) => {
            warn!("SVM run_test_guest: init failed: {:?}", e);
            loop {
                unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
            }
        }
    };

    let mut guest_code = match alloc_page4k_zeroed() {
        Ok(p) => p,
        Err(e) => {
            warn!("SVM run_test_guest: alloc code page failed: {:?}", e);
            loop {
                unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
            }
        }
    };
    let guest_stack = match alloc_page4k_zeroed() {
        Ok(p) => p,
        Err(e) => {
            warn!("SVM run_test_guest: alloc stack page failed: {:?}", e);
            loop {
                unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
            }
        }
    };

    unsafe {
        core::ptr::copy_nonoverlapping(
            GUEST_STUB.as_ptr(),
            guest_code.as_mut_ptr(),
            GUEST_STUB.len(),
        );
    }

    let guest_rip = guest_code.va().as_u64();
    let guest_rsp = (guest_stack.va().as_u64() + 4096 - 16) & !0xf;

    let host_efer = unsafe { rdmsr(MSR_EFER) };
    let guest_efer = host_efer | EFER_SVME;

    let host_cr0 = unsafe { read_cr0() };
    let host_cr4 = unsafe { read_cr4() };
    let (host_cr3_frame, _) = x86_64::registers::control::Cr3::read();
    let host_cr3 = host_cr3_frame.start_address().as_u64();
    let host_rflags = unsafe { read_rflags() };

    let gdtr = unsafe { sgdt() };
    let idtr = unsafe { sidt() };

    const ATTR_CODE64: u16 = 0x0A9B;
    const ATTR_DATA: u16 = 0x0C93;
    const ATTR_TSS_AVAIL: u16 = 0x0089;
    const FLAT_LIMIT: u32 = 0xFFFFF;

    ctx.vmcb.write_save_seg(
        vmcb::save::CS,
        gdt::GDT.kernel.code_selector.0,
        ATTR_CODE64,
        FLAT_LIMIT,
        0,
    );
    ctx.vmcb.write_save_seg(
        vmcb::save::SS,
        gdt::GDT.kernel.data_selector.0,
        ATTR_DATA,
        FLAT_LIMIT,
        0,
    );
    ctx.vmcb.write_save_seg(
        vmcb::save::DS,
        gdt::GDT.kernel.data_selector.0,
        ATTR_DATA,
        FLAT_LIMIT,
        0,
    );
    ctx.vmcb.write_save_seg(
        vmcb::save::ES,
        gdt::GDT.kernel.data_selector.0,
        ATTR_DATA,
        FLAT_LIMIT,
        0,
    );
    ctx.vmcb.write_save_seg(
        vmcb::save::FS,
        gdt::GDT.kernel.data_selector.0,
        ATTR_DATA,
        FLAT_LIMIT,
        0,
    );
    ctx.vmcb.write_save_seg(
        vmcb::save::GS,
        gdt::GDT.kernel.data_selector.0,
        ATTR_DATA,
        FLAT_LIMIT,
        0,
    );

    ctx.vmcb
        .write_save_seg(vmcb::save::GDTR, 0, 0, gdtr.limit as u32, gdtr.base);
    ctx.vmcb
        .write_save_seg(vmcb::save::IDTR, 0, 0, idtr.limit as u32, idtr.base);
    ctx.vmcb.write_save_seg(vmcb::save::LDTR, 0, 0, 0, 0);

    let tss_base = VirtAddr::from_ptr(&*gdt::TSS).as_u64();
    let tss_limit = (core::mem::size_of::<x86_64::structures::tss::TaskStateSegment>() - 1) as u32;
    ctx.vmcb.write_save_seg(
        vmcb::save::TR,
        gdt::GDT.tss_selector.0,
        ATTR_TSS_AVAIL,
        tss_limit,
        tss_base,
    );

    unsafe {
        ctx.vmcb.write_u8(vmcb::VMCB_SAVE_BASE + vmcb::save::CPL, 0);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::EFER, guest_efer);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::CR0, host_cr0);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::CR3, host_cr3);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::CR4, host_cr4);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::DR6, 0xffff_0ff0);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::DR7, 0x0000_0400);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::RFLAGS, host_rflags);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::RIP, guest_rip);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::RSP, guest_rsp);
        ctx.vmcb
            .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::RAX, 0);

        ctx.vmcb
            .write_u64(vmcb::control::IOPM_BASE_PA, ctx.iopm[0].pa());
        ctx.vmcb
            .write_u64(vmcb::control::MSRPM_BASE_PA, ctx.msrpm[0].pa());
        ctx.vmcb.write_u32(vmcb::control::ASID, 1);
        ctx.vmcb
            .write_u8(vmcb::control::TLB_CTL, vmcb::tlb_ctl::FLUSH_ASID);
        ctx.vmcb.write_u64(vmcb::control::NESTED_CTL, 0);

        ctx.vmcb.set_intercept(vmcb::intercept::VMRUN);
        ctx.vmcb.set_intercept(vmcb::intercept::VMMCALL);
        ctx.vmcb.set_intercept(vmcb::intercept::HLT);
    }

    info!(
        "SVM test guest: rip={:#x} rsp={:#x} vmcb_pa={:#x}",
        guest_rip, guest_rsp, ctx.vmcb_pa
    );

    x86_64::instructions::interrupts::disable();
    loop {
        unsafe {
            asm!(
                "vmrun",
                in("rax") ctx.vmcb_pa,
                clobber_abi("sysv64"),
                options(nostack),
            );
        }

        let code = unsafe { ctx.vmcb.read_u32(vmcb::control::EXIT_CODE) };
        match code {
            vmcb::exit_code::VMMCALL => {
                let next_rip = unsafe { ctx.vmcb.read_u64(vmcb::control::NEXT_RIP) };
                info!("SVM VMEXIT: VMMCALL next_rip={:#x}", next_rip);
                unsafe {
                    ctx.vmcb
                        .write_u64(vmcb::VMCB_SAVE_BASE + vmcb::save::RIP, next_rip);
                }
                info!("SVM guest resume: rip <= next_rip, re-entering VMRUN");
            }
            vmcb::exit_code::HLT => {
                info!("SVM VMEXIT: HLT (powering off)");
                qemu::shutdown(0);
                loop {
                    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
                }
            }
            vmcb::exit_code::NPF => {
                let info1 = unsafe { ctx.vmcb.read_u64(vmcb::control::EXIT_INFO_1) };
                let info2 = unsafe { ctx.vmcb.read_u64(vmcb::control::EXIT_INFO_2) };
                warn!(
                    "SVM VMEXIT: NPF exit_info_1={:#x} exit_info_2={:#x}",
                    info1, info2
                );
                qemu::shutdown(1);
                loop {
                    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
                }
            }
            vmcb::exit_code::ERR => {
                let info1 = unsafe { ctx.vmcb.read_u64(vmcb::control::EXIT_INFO_1) };
                let info2 = unsafe { ctx.vmcb.read_u64(vmcb::control::EXIT_INFO_2) };
                warn!(
                    "SVM VMEXIT: INVALID (likely bad VMCB guest state) exit_info_1={:#x} exit_info_2={:#x}",
                    info1, info2
                );
                qemu::shutdown(2);
                loop {
                    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
                }
            }
            other => {
                let info1 = unsafe { ctx.vmcb.read_u64(vmcb::control::EXIT_INFO_1) };
                let info2 = unsafe { ctx.vmcb.read_u64(vmcb::control::EXIT_INFO_2) };
                warn!(
                    "SVM VMEXIT: code={:#x} exit_info_1={:#x} exit_info_2={:#x}",
                    other, info1, info2
                );
                qemu::shutdown(3);
                loop {
                    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
                }
            }
        }
    }
}
