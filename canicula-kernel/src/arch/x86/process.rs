use lazy_static::lazy_static;
use spin::Mutex;

extern crate alloc;

lazy_static! {
    pub static ref TASKS: Mutex<alloc::vec::Vec<Task>> = Mutex::new(alloc::vec::Vec::new());
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Task {
    pub id: usize,
    pub stack: [u8; 4096],
    pub stack_pointer: usize,
    pub page_table: usize,
    pub entry: fn() -> !,
}
