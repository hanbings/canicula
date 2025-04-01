use lazy_static::lazy_static;
use log::error;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::arch::x86::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt.breakpoint.set_handler_fn(breakpoint_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    error!("double fault\n{:#?}", stack_frame);

    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    error!("exception: breakpoint\n{:#?}", stack_frame);
}
