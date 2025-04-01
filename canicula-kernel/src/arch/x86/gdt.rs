use lazy_static::lazy_static;
use x86_64::{
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 4;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_size = STACK_SIZE as u64;
            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + stack_size;
            stack_end
        };
        tss
    };
    static ref GDT: GdtFlush = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));

        GdtFlush {
            global_descriptor_table: gdt,
            selectors: Selectors {
                code_selector,
                tss_selector,
            },
        }
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

struct GdtFlush {
    global_descriptor_table: GlobalDescriptorTable,
    selectors: Selectors,
}

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    GDT.global_descriptor_table.load();
    unsafe {
        CS::set_reg(GDT.selectors.code_selector);
        load_tss(GDT.selectors.tss_selector);
    }
}
