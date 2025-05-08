use core::iter::Map;

use lazy_static::lazy_static;

extern crate alloc;
use alloc::vec::Vec;

lazy_static! {
    pub static ref PROCESSES: Processes = Processes {
        processes: Vec::new(),
    };
}

#[allow(unused_variables)]
pub fn entry_point(args: &[&str]) {}

#[derive(Debug, Clone)]
pub struct Processes {
    // Processes list fot pre physical processor
    processes: Vec<Map<usize, ProcessControlBlock>>,
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
    Waiting,
    Stopped,
    Zombie,
    Terminated,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessRegister {
    eax: usize,
    ebx: usize,
    ecx: usize,
    edx: usize,
    esi: usize,
    edi: usize,
    ebp: usize,
    esp: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessControlBlock {
    process_id: usize,
    process_state: ProcessState,
    process_priority: usize,
    created_time: usize,
    group_id: usize,
    parent_id: usize,
    user_id: usize,
    exit_code: usize,

    entry_point: usize,

    page_table: usize,
    stack_pointer: usize,
    instruction_pointer: usize,
    register: ProcessRegister,
}

impl ProcessControlBlock {
    pub fn new(entry_point: usize) -> Self {
        ProcessControlBlock {
            process_id: 0,
            process_state: ProcessState::Running,
            process_priority: 0,
            created_time: 0,
            group_id: 0,
            parent_id: 0,
            user_id: 0,
            exit_code: 0,

            entry_point,

            page_table: 0,
            stack_pointer: 0,
            instruction_pointer: 0,
            register: ProcessRegister {
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: 0,
                esi: 0,
                edi: 0,
                ebp: 0,
                esp: 0,
            },
        }
    }
}

pub fn create_process(entry_point: usize) {}
pub fn distory_process(pid: usize) {}
pub fn switch_process(pid: usize) {}
pub fn wait_process(pid: usize) {}
pub fn exit_process(pid: usize, exit_code: usize) {}
pub fn poll_process() {}
