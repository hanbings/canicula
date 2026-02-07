pub mod heap_allocator;
pub mod page_allocator;

extern crate alloc;
use alloc::vec::Vec;

use lazy_static::lazy_static;
use log::info;
use spin::Once;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{PageTable, page_table::FrameError},
};

lazy_static! {
    static ref PHYSICAL_MEMORY_OFFSET: Once<VirtAddr> = Once::new();
}

pub unsafe fn physical_to_virtual(addr: PhysAddr) -> VirtAddr {
    let phys = PHYSICAL_MEMORY_OFFSET.get().unwrap();
    VirtAddr::new(phys.as_u64() + addr.as_u64())
}

pub unsafe fn virtual_to_physical(
    addr: VirtAddr,
    physical_memory_offset: VirtAddr,
) -> Option<PhysAddr> {
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];
    let mut frame = level_4_table_frame;

    for &index in &table_indexes {
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };

        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    Some(frame.start_address() + u64::from(addr.page_offset()))
}

pub fn init(boot_info: &'static mut canicula_common::entry::BootInfo) -> &'static canicula_common::entry::BootInfo {
    let physical_memory_offset =
        VirtAddr::new(boot_info.physical_memory_offset.expect("Physical memory offset not found"));

    PHYSICAL_MEMORY_OFFSET.call_once(|| physical_memory_offset);

    let (mut mapper, mut frame_allocator, boot_info) = page_allocator::init(boot_info);
    let _ = heap_allocator::init(&mut mapper, &mut frame_allocator);

    boot_info
}

pub fn alloc_test() {
    info!("Running bumb tests...");

    let mut pool = Vec::new();

    for i in 0..8 {
        info!("Indicator: {}", i);
        let mut items = alloc_pass(i);
        free_pass(&mut items, i as u8);

        pool.append(&mut items);
        assert_eq!(items.len(), 0);
    }

    info!("Bumb tests run OK!");
}

fn alloc_pass(delta: usize) -> Vec<Vec<u8>> {
    let mut items = Vec::new();
    let mut base = 32;
    loop {
        let c = (delta % 256) as u8;
        let a = alloc::vec![c; base+delta];
        items.push(a);
        if base >= 512 * 1024 {
            break;
        }
        base *= 2;
    }
    items
}

fn free_pass(items: &mut Vec<Vec<u8>>, delta: u8) {
    let total = items.len();
    for j in (0..total).rev() {
        if j % 2 == 0 {
            let ret = items.remove(j);
            assert_eq!(delta, ret[0]);
            assert_eq!(delta, ret[ret.len() - 1]);
        }
    }
}
