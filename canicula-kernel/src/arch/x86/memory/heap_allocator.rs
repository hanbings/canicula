use linked_list_allocator::LockedHeap;
use log::{debug, error};
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError,
    },
};

extern crate alloc;
use core::alloc::Layout;

pub const HEAP_START: usize = 0x_ffff_a000_0000_0000;
pub const HEAP_SIZE: usize = 32 * 1024 * 1024;

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    error!("allocation error: {:?}", layout);

    loop {
        x86_64::instructions::hlt();
    }
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE.try_into().unwrap() - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        let heap_start = HEAP_START as *mut u8;
        let heap_size = HEAP_SIZE;

        debug!(
            "Heap start: {:#x}, size: {}",
            heap_start as usize, heap_size
        );
        ALLOCATOR.lock().init(heap_start, heap_size);
    }

    Ok(())
}
