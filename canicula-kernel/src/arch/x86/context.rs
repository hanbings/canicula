/// Saved CPU context for a kernel task.
///
/// Only the stack pointer is stored here. The actual callee-saved registers
/// (rbx, rbp, r12-r15) are pushed onto the task's kernel stack by
/// `context_switch` and popped when the task is resumed.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TaskContext {
    pub rsp: u64,
}

impl TaskContext {
    pub const fn empty() -> Self {
        TaskContext { rsp: 0 }
    }
}

/// Switch execution from the current task to a new task.
///
/// Saves callee-saved registers onto the current stack, stores RSP into
/// `old.rsp`, loads RSP from `new.rsp`, restores callee-saved registers
/// from the new stack, and returns into the new task's execution.
///
/// # Safety
///
/// - `old` must point to the current task's valid TaskContext.
/// - `new` must point to the target task's valid TaskContext whose stack
///   has been properly initialized (either by a previous context_switch
///   or by `Task::new`).
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(old: *mut TaskContext, new: *const TaskContext) {
    // System V AMD64 ABI: rdi = old, rsi = new
    core::arch::naked_asm!(
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // Save current RSP into old->rsp
        "mov [rdi], rsp",
        // Load new task's RSP from new->rsp
        "mov rsp, [rsi]",
        // Restore callee-saved registers from the new task's stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        // Return into the new task (pops return address from stack)
        "ret",
    )
}
