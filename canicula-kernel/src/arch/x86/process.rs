use log::info;
use spin::Mutex;

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use x86_64::registers::control::Cr3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Zombie,
    Exited,
}

pub struct ProcessControlBlock {
    pub pid: u64,
    pub name: &'static str,
    pub state: ProcessState,
    pub parent_pid: Option<u64>,
    pub threads: Vec<u64>,
    pub exit_code: Option<i32>,
    /// Physical address of the PML4 page table (CR3 value).
    /// Currently all processes share the kernel page table.
    pub page_table: u64,
}

pub struct ProcessTable {
    processes: BTreeMap<u64, ProcessControlBlock>,
    next_pid: u64,
}

impl ProcessTable {
    pub const fn new() -> Self {
        ProcessTable {
            processes: BTreeMap::new(),
            next_pid: 0,
        }
    }

    pub fn alloc_pid(&mut self) -> u64 {
        let pid = self.next_pid;
        self.next_pid += 1;
        pid
    }

    pub fn insert(&mut self, pcb: ProcessControlBlock) {
        self.processes.insert(pcb.pid, pcb);
    }

    pub fn get(&self, pid: u64) -> Option<&ProcessControlBlock> {
        self.processes.get(&pid)
    }

    pub fn get_mut(&mut self, pid: u64) -> Option<&mut ProcessControlBlock> {
        self.processes.get_mut(&pid)
    }

    pub fn len(&self) -> usize {
        self.processes.len()
    }
}

pub static PROCESS_TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());

/// Create a new process with a main thread executing `entry_fn`.
/// Returns the new process's PID.
pub fn create_process(name: &'static str, entry_fn: fn() -> !) -> u64 {
    let pid = {
        let mut table = PROCESS_TABLE.lock();
        let pid = table.alloc_pid();
        let (cr3_frame, _) = Cr3::read();
        let page_table = cr3_frame.start_address().as_u64();

        let pcb = ProcessControlBlock {
            pid,
            name,
            state: ProcessState::Running,
            parent_pid: if pid == 0 { None } else { Some(0) },
            threads: Vec::new(),
            exit_code: None,
            page_table,
        };
        table.insert(pcb);
        pid
    };
    // PROCESS_TABLE lock is dropped here

    // Spawn the main thread for this process
    let tid = super::scheduler::spawn_thread(pid, entry_fn);

    // Record the TID in the PCB
    {
        let mut table = PROCESS_TABLE.lock();
        if let Some(pcb) = table.get_mut(pid) {
            pcb.threads.push(tid);
        }
    }

    info!(
        "Process {} ({}) created with main thread {}",
        pid, name, tid
    );
    pid
}

/// Mark a process as Zombie and all its threads as Exited.
pub fn exit_process(pid: u64, exit_code: i32) {
    let thread_ids = {
        let mut table = PROCESS_TABLE.lock();
        if let Some(pcb) = table.get_mut(pid) {
            pcb.state = ProcessState::Zombie;
            pcb.exit_code = Some(exit_code);
            pcb.threads.clone()
        } else {
            return;
        }
    };

    // Mark all threads as exited
    for tid in thread_ids {
        super::scheduler::mark_thread_exited(tid);
    }

    info!("Process {} exited with code {}", pid, exit_code);
}

/// Returns the PID of the currently running thread's process.
pub fn current_pid() -> u64 {
    super::scheduler::current_pid()
}

/// Initialize the process table. Must be called after heap init.
pub fn init() {
    // Process table is ready (const-initialized).
    // PID 0 (kernel process) will be created by scheduler::init().
    info!("Process table initialized");
}
