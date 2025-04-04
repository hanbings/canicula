use spin::Lazy;

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

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

pub static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.append(Descriptor::kernel_code_segment());
    let data_selector = gdt.append(Descriptor::kernel_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
    (
        gdt,
        Selectors {
            code_selector,
            data_selector,
            tss_selector,
        },
    )
});

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS, SS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        DS::set_reg(GDT.1.data_selector);
        SS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
}
