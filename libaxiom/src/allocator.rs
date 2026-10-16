//! AxiomOS User-Space Heap Allocator
//!
//! Provides dynamic memory allocation (malloc / free semantics) in Ring 3 
//! entirely in user space without kernel trapping or ambient authority.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

pub const DEFAULT_HEAP_SIZE: usize = 64 * 1024; // 64 KiB default user heap

/// A thread-safe, lock-free bump allocator for user-space tasks.
pub struct BumpAllocator {
    heap_start: usize,
    heap_size: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_size: 0,
            next: AtomicUsize::new(0),
        }
    }

    /// Initializes the allocator with a designated memory region.
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.heap_start = start;
        self.heap_size = size;
        self.next.store(start, Ordering::SeqCst);
    }

    /// Allocates a block of memory with the specified layout.
    pub fn allocate(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        loop {
            let current = self.next.load(Ordering::SeqCst);
            let remainder = current % align;
            let aligned = if remainder == 0 { current } else { current + (align - remainder) };
            let new_next = aligned + size;

            if new_next > self.heap_start + self.heap_size {
                return core::ptr::null_mut(); // Out of memory
            }

            if self.next.compare_exchange(current, new_next, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                return aligned as *mut u8;
            }
        }
    }

    /// Deallocation in a bump allocator is a no-op (freed on bulk reset).
    pub fn deallocate(&self, _ptr: *mut u8, _layout: Layout) {}
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.deallocate(ptr, layout);
    }
}
