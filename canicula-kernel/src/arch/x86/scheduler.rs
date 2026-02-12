use core::sync::atomic::{AtomicU64, Ordering};
use log::info;
use spin::Mutex;

use super::context::{TaskContext, context_switch};
use super::process;

extern crate alloc;
use alloc::vec::Vec;

const KERNEL_STACK_SIZE: usize = 4096 * 4;

static NEXT_TID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Exited,
}

/// Thread Control Block. The scheduler schedules threads, not processes.
/// Each thread belongs to exactly one process (identified by `pid`).
pub struct ThreadControlBlock {
    pub tid: u64,
    pub pid: u64,
    pub state: ThreadState,
    pub context: TaskContext,
    kernel_stack: Vec<u8>,
}

impl ThreadControlBlock {
    /// Create a new thread for the given process.
    pub fn new(pid: u64, entry_fn: fn() -> !) -> Self {
        let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);
        let kernel_stack = alloc::vec![0u8; KERNEL_STACK_SIZE];

        let stack_top = kernel_stack.as_ptr() as u64 + KERNEL_STACK_SIZE as u64;
        let stack_top = stack_top & !0xF;

        // Initial stack layout (growing downward from stack_top):
        //
        //   stack_top -  8  -> ret_addr = thread_trampoline
        //   stack_top - 16  -> rbx = 0
        //   stack_top - 24  -> rbp = 0
        //   stack_top - 32  -> r12 = entry_fn
        //   stack_top - 40  -> r13 = 0
        //   stack_top - 48  -> r14 = 0
        //   stack_top - 56  -> r15 = 0   <- rsp points here
        //
        // context_switch pops: r15, r14, r13, r12, rbp, rbx, then ret.
        let rsp = stack_top - 7 * 8;

        unsafe {
            let ptr = rsp as *mut u64;
            *ptr.add(0) = 0; // r15
            *ptr.add(1) = 0; // r14
            *ptr.add(2) = 0; // r13
            *ptr.add(3) = entry_fn as *const () as u64; // r12
            *ptr.add(4) = 0; // rbp
            *ptr.add(5) = 0; // rbx
            *ptr.add(6) = thread_trampoline as *const () as u64; // return address
        }

        ThreadControlBlock {
            tid,
            pid,
            state: ThreadState::Ready,
            context: TaskContext { rsp },
            kernel_stack,
        }
    }
}

/// Trampoline for first entry into a new thread.
///
/// Entered via `ret` from context_switch. r12 holds the entry function pointer.
/// Interrupts are disabled (from the timer handler), so we must re-enable them.
#[unsafe(naked)]
unsafe extern "C" fn thread_trampoline() {
    core::arch::naked_asm!("sti", "call r12", "ud2",)
}

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub struct Scheduler {
    threads: Vec<ThreadControlBlock>,
    current: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            threads: Vec::new(),
            current: 0,
        }
    }

    pub fn add_thread(&mut self, tcb: ThreadControlBlock) {
        self.threads.push(tcb);
    }

    /// Round-robin: find the next Ready thread, skipping Exited and Blocked.
    fn next_ready_thread(&self) -> Option<usize> {
        let n = self.threads.len();
        if n <= 1 {
            return None;
        }
        for i in 1..n {
            let idx = (self.current + i) % n;
            if self.threads[idx].state == ThreadState::Ready {
                return Some(idx);
            }
        }
        None
    }

    pub fn prepare_switch(&mut self) -> Option<(*mut TaskContext, *const TaskContext)> {
        let next_idx = self.next_ready_thread()?;
        let current_idx = self.current;

        if self.threads[current_idx].state == ThreadState::Running {
            self.threads[current_idx].state = ThreadState::Ready;
        }
        self.threads[next_idx].state = ThreadState::Running;
        self.current = next_idx;

        let old_ctx = &mut self.threads[current_idx].context as *mut TaskContext;
        let new_ctx = &self.threads[next_idx].context as *const TaskContext;

        Some((old_ctx, new_ctx))
    }

    /// Get the currently running thread's TID.
    pub fn current_tid(&self) -> u64 {
        self.threads[self.current].tid
    }

    /// Get the currently running thread's PID.
    pub fn current_pid(&self) -> u64 {
        self.threads[self.current].pid
    }

    /// Mark a thread as Exited by TID.
    pub fn mark_exited(&mut self, tid: u64) {
        for thread in &mut self.threads {
            if thread.tid == tid {
                thread.state = ThreadState::Exited;
                return;
            }
        }
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

/// Called from the timer interrupt handler.
pub fn tick() {
    let switch_info = {
        let mut sched = SCHEDULER.lock();
        sched.prepare_switch()
    };

    if let Some((old_ctx, new_ctx)) = switch_info {
        unsafe {
            context_switch(old_ctx, new_ctx);
        }
    }
}

/// Spawn a new thread for a process and add it to the scheduler.
/// Returns the new thread's TID.
pub fn spawn_thread(pid: u64, entry_fn: fn() -> !) -> u64 {
    let tcb = ThreadControlBlock::new(pid, entry_fn);
    let tid = tcb.tid;
    SCHEDULER.lock().add_thread(tcb);
    tid
}

/// Mark a thread as Exited.
pub fn mark_thread_exited(tid: u64) {
    SCHEDULER.lock().mark_exited(tid);
}

/// Get the TID of the currently running thread.
pub fn current_tid() -> u64 {
    SCHEDULER.lock().current_tid()
}

/// Get the PID of the currently running thread's process.
pub fn current_pid() -> u64 {
    SCHEDULER.lock().current_pid()
}

// Demo tasks

fn demo_task_a() -> ! {
    let pid = current_pid();
    let tid = current_tid();
    let mut count: u64 = 0;
    loop {
        if count % 10_000_000 == 0 {
            info!(
                "Process {} / Thread {}: tick {}",
                pid,
                tid,
                count / 10_000_000
            );
        }
        count += 1;
    }
}

fn demo_task_b() -> ! {
    let pid = current_pid();
    let tid = current_tid();
    let mut count: u64 = 0;
    loop {
        if count % 10_000_000 == 0 {
            info!(
                "Process {} / Thread {}: tick {}",
                pid,
                tid,
                count / 10_000_000
            );
        }
        count += 1;
    }
}

/// Initialize the scheduler.
///
/// Must be called after process::init() and heap init, before interrupts are enabled.
pub fn init() {
    // Create PID 0 kernel process with idle thread (TID 0)
    {
        let mut table = process::PROCESS_TABLE.lock();
        let pid = table.alloc_pid(); // PID 0

        let (cr3_frame, _) = x86_64::registers::control::Cr3::read();
        let pcb = process::ProcessControlBlock {
            pid,
            name: "kernel",
            state: process::ProcessState::Running,
            parent_pid: None,
            threads: alloc::vec![0],
            exit_code: None,
            page_table: cr3_frame.start_address().as_u64(),
        };
        table.insert(pcb);
    }

    // TID 0: idle thread (boot context, no separate stack)
    {
        let idle = ThreadControlBlock {
            tid: 0,
            pid: 0,
            state: ThreadState::Running,
            context: TaskContext::empty(),
            kernel_stack: Vec::new(),
        };
        SCHEDULER.lock().add_thread(idle);
    }

    // Create demo processes
    process::create_process("demo_a", demo_task_a);
    process::create_process("demo_b", demo_task_b);

    let sched = SCHEDULER.lock();
    info!(
        "Scheduler initialized with {} threads",
        sched.thread_count()
    );
}
