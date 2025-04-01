use log::debug;
use x86_64::registers::control::Cr3;
use x86_64::registers::debug;
use x86_64::structures::paging::page_table::FrameError;
use x86_64::structures::paging::PageTable;
use x86_64::{PhysAddr, VirtAddr};

pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
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

pub fn init(boot_info: &mut bootloader_api::BootInfo) {
    debug!("boot info {:?}", boot_info);

    let phys_mem_offset =
        VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (index, entry) in l4_table.iter().enumerate() {
        if !entry.is_unused() {
            debug!("Level 4 Table Entry {}: {:?}", index, entry);
        }
    }

    for region in boot_info.memory_regions.iter() {
        let start = VirtAddr::new(region.start);
        let end = VirtAddr::new(region.end);
        let size = end - start;
        let region_type = region.kind;
        
        debug!(
            "Memory region: 0x{:x} - 0x{:x} ({:x} bytes) - {:?}",
            start.as_u64(),
            end.as_u64(),
            size,
            region_type
        );
    }
}
