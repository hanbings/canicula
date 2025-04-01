use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use log::debug;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::page_table::FrameError;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

pub struct AbyssFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl AbyssFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        AbyssFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for AbyssFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

#[allow(dead_code)]
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

pub fn init(
    boot_info: &'static mut bootloader_api::BootInfo,
) -> (OffsetPageTable<'static>, AbyssFrameAllocator) {
    debug!("boot info {:?}", boot_info);

    let physical_memory_offset =
        VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let l4_table = unsafe { active_level_4_table(physical_memory_offset) };

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

    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        let table = OffsetPageTable::new(level_4_table, physical_memory_offset);
        let frame_allocator = AbyssFrameAllocator::init(&boot_info.memory_regions);

        (table, frame_allocator)
    }
}
