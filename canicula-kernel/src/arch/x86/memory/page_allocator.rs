use canicula_common::entry::{MemoryRegionKind, MemoryRegions};
use log::debug;
use x86_64::registers::control::Cr3;
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
        const MIN_USABLE_PA: u64 = 0x0010_0000;
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        let frame_addresses = addr_ranges
            .flat_map(|r| r.step_by(4096))
            .filter(|&addr| (addr as u64) >= MIN_USABLE_PA);
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

pub fn init(
    boot_info: &'static mut canicula_common::entry::BootInfo,
) -> (
    OffsetPageTable<'static>,
    AbyssFrameAllocator,
    &'static canicula_common::entry::BootInfo,
) {
    debug!("boot info {:#?}", boot_info);

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset.unwrap());

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

        (table, frame_allocator, boot_info)
    }
}
