use core::hint::spin_loop;
use core::sync::atomic::{AtomicU32, Ordering};

extern crate alloc;
use alloc::boxed::Box;

use acpi::{
    AcpiTables,
    platform::AcpiPlatform,
};
use log::{info, warn};
use x86_64::PhysAddr;

use crate::serial_println;

use super::apic::LAPIC;
use super::memory::physical_to_virtual;
use super::smp_trampoline::{AP_TRAMPOLINE_DATA_OFFSET, trampoline_bytes};

const AP_STACK_SIZE: usize = 4096 * 8; // 32 KiB

#[repr(C)]
#[derive(Clone, Copy)]
struct ApTrampolineData {
    cr3_low: u32,
    _reserved0: u32,
    entry: u64,
    stack_top: u64,
    cpu_id: u32,
    apic_id: u32,
    ack: u32,
    _reserved1: u32,
}

const _: () = {
    assert!(core::mem::size_of::<ApTrampolineData>() == 40);
};

static AP_ONLINE_COUNT: AtomicU32 = AtomicU32::new(0);

fn io_delay() {
    use x86_64::instructions::port::Port;
    unsafe { Port::<u8>::new(0x80).write(0) };
}

fn udelay(mut us: u64) {
    while us > 0 {
        io_delay();
        us -= 1;
    }
}

fn mdelay(ms: u64) {
    udelay(ms * 1000);
}

unsafe fn write_trampoline_page(trampoline_phys: u64) {
    let bytes = trampoline_bytes();
    assert!(bytes.len() <= 4096, "trampoline section exceeds 4KiB");

    let dst = unsafe { physical_to_virtual(PhysAddr::new(trampoline_phys)).as_mut_ptr::<u8>() };
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
        // Zero the rest of the page to avoid stale data.
        core::ptr::write_bytes(dst.add(bytes.len()), 0, 4096 - bytes.len());
    }
}

unsafe fn trampoline_data_ptr(trampoline_phys: u64) -> *mut ApTrampolineData {
    let data_phys = trampoline_phys + AP_TRAMPOLINE_DATA_OFFSET as u64;
    let v = unsafe { physical_to_virtual(PhysAddr::new(data_phys)) };
    v.as_mut_ptr::<ApTrampolineData>()
}

/// Bring up APs (x86_64 SMP) using INIT+SIPI and a 4KiB trampoline page.
///
/// This is intentionally minimal: APs will print a message and then halt.
pub fn init(boot_info: &'static canicula_common::entry::BootInfo) {
    let Some(trampoline_phys) = boot_info.smp_trampoline else {
        warn!("SMP: no trampoline page provided by loader, skipping AP bring-up");
        return;
    };

    // Only run on BSP.
    let is_bsp = unsafe {
        #[allow(static_mut_refs)]
        LAPIC
            .get()
            .map(|l| l.lock().is_bsp())
            .unwrap_or(true)
    };
    if !is_bsp {
        return;
    }

    let rsdp = match boot_info.rsdp_addr {
        Some(r) => r,
        None => {
            warn!("SMP: no RSDP in BootInfo, skipping AP bring-up");
            return;
        }
    };

    // Copy trampoline bytes into the low page.
    unsafe { write_trampoline_page(trampoline_phys) };

    // Parse ACPI to enumerate processors.
    let handler = crate::arch::x86::acpi::handler::AcpiHandler;
    let tables = match unsafe { AcpiTables::from_rsdp(handler, rsdp as usize) } {
        Ok(t) => t,
        Err(e) => {
            warn!("SMP: failed to parse ACPI tables: {:?}", e);
            return;
        }
    };
    let platform = match AcpiPlatform::new(tables, handler) {
        Ok(p) => p,
        Err(e) => {
            warn!("SMP: failed to build ACPI platform: {:?}", e);
            return;
        }
    };

    let Some(proc_info) = platform.processor_info else {
        warn!("SMP: ACPI has no processor_info, skipping AP bring-up");
        return;
    };

    info!(
        "SMP: BSP apic_id={} (uid={}), APs={}",
        proc_info.boot_processor.local_apic_id,
        proc_info.boot_processor.processor_uid,
        proc_info.application_processors.len()
    );

    // Read current CR3 (PML4 physical address).
    let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
    let cr3_phys = cr3_frame.start_address().as_u64();
    if cr3_phys > u32::MAX as u64 {
        warn!("SMP: CR3 is above 4GiB ({:#x}), trampoline expects low CR3; skipping", cr3_phys);
        return;
    }

    for (cpu_index, proc) in proc_info.application_processors.iter().enumerate() {
        if proc.state == acpi::platform::ProcessorState::Disabled {
            continue;
        }

        let apic_id = proc.local_apic_id;
        let cpu_id = (cpu_index as u32) + 1; // BSP = 0

        // Allocate and leak a kernel stack for this AP.
        let stack: &'static mut [u8] =
            Box::leak(alloc::vec![0u8; AP_STACK_SIZE].into_boxed_slice());
        let mut stack_top = stack.as_ptr() as u64 + stack.len() as u64;
        stack_top &= !0xF; // 16-byte align

        let vector = (trampoline_phys >> 12) as u8;
        info!("SMP: starting AP cpu_id={} apic_id={} vector={:#x}", cpu_id, apic_id, vector);

        unsafe {
            // Fill AP data using volatile write so the compiler can't assume it never changes.
            let data_ptr = trampoline_data_ptr(trampoline_phys);
            core::ptr::write_volatile(
                data_ptr,
                ApTrampolineData {
                    cr3_low: cr3_phys as u32,
                    _reserved0: 0,
                    entry: ap_rust_entry as usize as u64,
                    stack_top,
                    cpu_id,
                    apic_id,
                    ack: 0,
                    _reserved1: 0,
                },
            );
            core::sync::atomic::compiler_fence(Ordering::SeqCst);

            #[allow(static_mut_refs)]
            let mut lapic = LAPIC.get().unwrap().lock();

            // INIT IPI then SIPI. (Delays are conservative.)
            lapic.send_init_ipi(apic_id);
            mdelay(10);
            lapic.send_sipi(vector, apic_id);
            mdelay(1);
            lapic.send_sipi(vector, apic_id);
        }

        // Wait for AP to set ack.
        let ack_ptr = unsafe {
            let data_ptr = trampoline_data_ptr(trampoline_phys);
            core::ptr::addr_of!((*data_ptr).ack)
        };
        let mut timeout = 5_000_000u64;
        loop {
            let ack = unsafe { core::ptr::read_volatile(ack_ptr) };
            if ack != 0 {
                break;
            }
            if timeout == 0 {
                warn!("SMP: AP apic_id={} did not respond (timeout)", apic_id);
                break;
            }
            timeout -= 1;
            spin_loop();
        }
    }

    info!(
        "SMP: bring-up done, online_count={}",
        AP_ONLINE_COUNT.load(Ordering::Relaxed)
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn ap_rust_entry(cpu_id: u32, apic_id: u32) -> ! {
    // We intentionally keep APs minimal for now: no interrupts, no scheduling.
    x86_64::instructions::interrupts::disable();

    AP_ONLINE_COUNT.fetch_add(1, Ordering::Relaxed);
    serial_println!("AP online: cpu_id={} apic_id={}", cpu_id, apic_id);

    loop {
        x86_64::instructions::hlt();
    }
}

