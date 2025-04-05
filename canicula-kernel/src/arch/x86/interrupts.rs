use log::{debug, warn};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use lazy_static::lazy_static;

use crate::{arch::x86::qemu::exit_qemu, println, serial_println};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 32,
    Keyboard = 33,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    #[allow(dead_code)]
    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

macro_rules! simple_handlers {
    ($($name:ident => $info:expr),* $(,)?) => {
        $(
            pub extern "x86-interrupt" fn $name(
                stack_frame: InterruptStackFrame
            ) {
                panic!("EXCEPTION: {}\n{:#?}", $info, stack_frame);
            }
        )*
    };
}

macro_rules! error_code_handlers {
    ($($name:ident => $info:expr),* $(,)?) => {
        $(
            pub extern "x86-interrupt" fn $name(
                stack_frame: InterruptStackFrame,
                error_code: u64
            ) {
                panic!(
                    "EXCEPTION: {} - ERROR CODE: {}\n{:#?}",
                    $info, error_code, stack_frame
                );
            }
        )*
    };
}

#[rustfmt::skip]
const SIMPLE_HANDLERS: &[(
    fn(&mut InterruptDescriptorTable) -> &mut x86_64::structures::idt::Entry<extern "x86-interrupt" fn(InterruptStackFrame)>,
    extern "x86-interrupt" fn(InterruptStackFrame),
)] = &[
    (|idt| &mut idt.divide_error, divide_by_zero_handler),
    (|idt| &mut idt.debug, debug_handler),
    (|idt| &mut idt.non_maskable_interrupt, non_maskable_interrupt_handler),
    (|idt| &mut idt.overflow, overflow_handler),
    (|idt| &mut idt.bound_range_exceeded, bound_range_exceeded_handler),
    (|idt| &mut idt.invalid_opcode, invalid_opcode_handler),
    (|idt| &mut idt.device_not_available, device_not_available_handler),
    (|idt| &mut idt.x87_floating_point, x87_floating_point_handler),
    (|idt| &mut idt.simd_floating_point, simd_floating_point_handler),
    (|idt| &mut idt.virtualization, virtualization_handler),
    (|idt| &mut idt.breakpoint, breakpoint_handler),
];

#[rustfmt::skip]
const ERROR_CODE_HANDLERS: &[(
    fn(&mut InterruptDescriptorTable) -> &mut x86_64::structures::idt::Entry<extern "x86-interrupt" fn(InterruptStackFrame, u64)>,
    extern "x86-interrupt" fn(InterruptStackFrame, u64),
)] = &[
    (|idt| &mut idt.invalid_tss, invalid_tss_handler),
    (|idt| &mut idt.segment_not_present, segment_not_present_handler),
    (|idt| &mut idt.stack_segment_fault, stack_segment_fault_handler),
    (|idt| &mut idt.general_protection_fault, general_protection_fault_handler),
    (|idt| &mut idt.alignment_check, alignment_check_handler),
    (|idt| &mut idt.security_exception, security_exception_handler),
];

simple_handlers!(
    divide_by_zero_handler          => "DIVIDE BY ZERO",
    debug_handler                   => "DEBUG",
    non_maskable_interrupt_handler  => "NON MASKABLE INTERRUPT",
    overflow_handler                => "OVERFLOW",
    bound_range_exceeded_handler    => "BOUND RANGE EXCEEDED",
    invalid_opcode_handler          => "INVALID OPCODE",
    device_not_available_handler    => "DEVICE NOT AVAILABLE",
    x87_floating_point_handler      => "X87 FLOATING POINT",
    simd_floating_point_handler     => "SIMD FLOATING POINT",
    virtualization_handler          => "VIRTUALIZATION",
);

error_code_handlers!(
    invalid_tss_handler                 => "INVALID TSS",
    segment_not_present_handler         => "SEGMENT NOT PRESENT",
    stack_segment_fault_handler         => "STACK SEGMENT FAULT",
    general_protection_fault_handler    => "GENERAL PROTECTION FAULT",
    alignment_check_handler             => "ALIGNMENT CHECK",
    security_exception_handler          => "SECURITY EXCEPTION",
);

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        for (getter, handler) in SIMPLE_HANDLERS {
            getter(&mut idt).set_handler_fn(*handler);
        }

        for (getter, handler) in ERROR_CODE_HANDLERS {
            getter(&mut idt).set_handler_fn(*handler);
        }

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::arch::x86::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        #[rustfmt::skip]
        idt.machine_check.set_handler_fn(machine_check_handler);
        #[rustfmt::skip]
        idt.page_fault.set_handler_fn(page_fault_handler);

        #[rustfmt::skip]
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        #[rustfmt::skip]
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "EXCEPTION: DOUBLE FAULT - ERROR CODE: {}\n{:#?}",
        error_code, stack_frame
    );
}

pub extern "x86-interrupt" fn machine_check_handler(stack_frame: InterruptStackFrame) -> ! {
    panic!("EXCEPTION: MACHINE CHECK\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    panic!(
        "EXCEPTION: PAGE FAULT - ERROR CODE: {:?}\nAccessed Address: {:?}\n{:#?}",
        error_code,
        Cr2::read(),
        stack_frame
    );
}

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);

    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use crate::arch::x86::apic::LAPIC;

    unsafe {
        #[allow(static_mut_refs)]
        LAPIC.get().unwrap().lock().end_interrupts();
    }
}

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    use crate::arch::x86::apic::LAPIC;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    warn!("Keyboard scancode: {}", scancode);
    if scancode == 28 {
        exit_qemu(0x10);
    }

    unsafe {
        #[allow(static_mut_refs)]
        LAPIC.get().unwrap().lock().end_interrupts();
    }
}

pub fn init() {
    IDT.load();
}

pub fn enable_interrupts() {
    debug!("Enabling interrupts");
    x86_64::instructions::interrupts::enable();
    debug!("Interrupts enabled");
}
