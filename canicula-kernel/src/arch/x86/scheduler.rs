use core::sync::atomic::{AtomicU64, Ordering};
use log::info;
use spin::Mutex;

use super::context::{TaskContext, context_switch};

extern crate alloc;
use alloc::vec::Vec;

const KERNEL_STACK_SIZE: usize = 4096 * 4;

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
}

pub struct Task {
    pub id: u64,
    pub state: TaskState,
    pub context: TaskContext,
    kernel_stack: Vec<u8>,
}

impl Task {
    pub fn new(entry_fn: fn() -> !) -> Self {
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let kernel_stack = alloc::vec![0u8; KERNEL_STACK_SIZE];

        let stack_top = kernel_stack.as_ptr() as u64 + KERNEL_STACK_SIZE as u64;
        let stack_top = stack_top & !0xF;

        // Initial stack layout (growing downward from stack_top):
        //
        //   stack_top -  8  -> ret_addr = task_trampoline
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
            *ptr.add(0) = 0;                          // r15
            *ptr.add(1) = 0;                          // r14
            *ptr.add(2) = 0;                          // r13
            *ptr.add(3) = entry_fn as *const () as u64; // r12
            *ptr.add(4) = 0;                          // rbp
            *ptr.add(5) = 0;                          // rbx
            *ptr.add(6) = task_trampoline as *const () as u64; // return address
        }

        Task {
            id,
            state: TaskState::Ready,
            context: TaskContext { rsp },
            kernel_stack,
        }
    }
}

/// Trampoline for first entry into a new task.
///
/// Entered via `ret` from context_switch. At this point r12 holds the
/// entry function pointer and interrupts are disabled (from the timer handler).
#[unsafe(naked)]
unsafe extern "C" fn task_trampoline() {
    core::arch::naked_asm!(
        "sti",
        "call r12",
        "ud2",
    )
}

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub struct Scheduler {
    tasks: Vec<Task>,
    current: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            tasks: Vec::new(),
            current: 0,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    fn next_ready_task(&self) -> Option<usize> {
        let n = self.tasks.len();
        if n <= 1 {
            return None;
        }
        for i in 1..n {
            let idx = (self.current + i) % n;
            if self.tasks[idx].state == TaskState::Ready {
                return Some(idx);
            }
        }
        None
    }

    /// Determine if a switch is needed. If so, update task states and return
    /// raw pointers to the two TaskContexts.
    ///
    /// The caller MUST drop the Scheduler lock before calling context_switch
    /// with these pointers.
    pub fn prepare_switch(&mut self) -> Option<(*mut TaskContext, *const TaskContext)> {
        let next_idx = self.next_ready_task()?;
        let current_idx = self.current;

        if self.tasks[current_idx].state == TaskState::Running {
            self.tasks[current_idx].state = TaskState::Ready;
        }
        self.tasks[next_idx].state = TaskState::Running;
        self.current = next_idx;

        let old_ctx = &mut self.tasks[current_idx].context as *mut TaskContext;
        let new_ctx = &self.tasks[next_idx].context as *const TaskContext;

        Some((old_ctx, new_ctx))
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

fn demo_task_a() -> ! {
    let mut count: u64 = 0;
    loop {
        if count % 10_000_000 == 0 {
            info!("Task A: tick {}", count / 10_000_000);
        }
        count += 1;
    }
}

fn demo_task_b() -> ! {
    let mut count: u64 = 0;
    loop {
        if count % 10_000_000 == 0 {
            info!("Task B: tick {}", count / 10_000_000);
        }
        count += 1;
    }
}

/// Initialize the scheduler with an idle task and demo tasks.
///
/// Must be called after heap init and before interrupts are enabled.
pub fn init() {
    let mut sched = SCHEDULER.lock();

    // Task 0: the current boot context. Uses the boot kernel stack.
    // Its context.rsp will be filled in by the first context_switch.
    let idle = Task {
        id: 0,
        state: TaskState::Running,
        context: TaskContext::empty(),
        kernel_stack: Vec::new(),
    };
    sched.add_task(idle);
    sched.add_task(Task::new(demo_task_a));
    sched.add_task(Task::new(demo_task_b));

    info!("Scheduler initialized with {} tasks", sched.tasks.len());
}
