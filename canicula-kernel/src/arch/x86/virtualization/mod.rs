// intel virtualization
pub mod vmx;
pub mod vmcs;

// amd virtualization
pub mod svm;
pub mod vmcb;

pub fn list_virtual_machines() {}
pub fn create_virtual_machine() {}
pub fn destroy_virtual_machine() {}
pub fn start_virtual_machine() {}
pub fn stop_virtual_machine() {}
pub fn suspend_virtual_machine() {}
pub fn resume_virtual_machine() {}