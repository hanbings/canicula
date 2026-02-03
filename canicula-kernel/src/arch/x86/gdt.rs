use spin::Lazy;

use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

use core::ptr::addr_of;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const DEBUG_IST_INDEX: u16 = 1;
pub const NON_MASKABLE_INTERRUPT_IST_INDEX: u16 = 2;

pub static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    let frames = [
        0,
        DOUBLE_FAULT_IST_INDEX,
        DEBUG_IST_INDEX,
        NON_MASKABLE_INTERRUPT_IST_INDEX,
    ];
    for (i, &_frame) in frames.iter().enumerate() {
        tss.interrupt_stack_table[i] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            #[allow(unused_unsafe)]
            let stack_start = VirtAddr::from_ptr(unsafe { addr_of!(STACK) });
            stack_start + STACK_SIZE as u64
        };
    }
    tss
});

pub static GDT: Lazy<Gdt> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
    let user_code_selector = gdt.append(Descriptor::user_code_segment());
    let user_data_selector = gdt.append(Descriptor::user_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));

    Gdt {
        gdt,
        tss_selector,
        kernel: Selectors {
            code_selector: kernel_code_selector,
            data_selector: kernel_data_selector,
        },
        user: Selectors {
            code_selector: user_code_selector,
            data_selector: user_data_selector,
        },
    }
});

pub struct Gdt {
    pub gdt: GlobalDescriptorTable,
    pub tss_selector: SegmentSelector,
    pub kernel: Selectors,
    pub user: Selectors,
}

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, SS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.gdt.load();
    unsafe {
        CS::set_reg(GDT.kernel.code_selector);
        DS::set_reg(GDT.kernel.data_selector);
        SS::set_reg(GDT.kernel.data_selector);
        load_tss(GDT.tss_selector);
    }
}
