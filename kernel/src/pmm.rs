// kernel/src/pmm.rs

use crate::E820Entry;

const PAGE_SIZE: u64 = 4096;

/// Simple bump allocator for physical frames
pub struct PhysicalMemoryManager {
    current: u64,
    end: u64,
}

impl PhysicalMemoryManager {
    /// Initialize PMM from BIOS E820 map
    pub fn new(e820: &[E820Entry]) -> Option<Self> {
        let mut best_base = 0;
        let mut best_len = 0;

        // Pick the largest usable region above 1 MiB
        for e in e820 {
            if e.kind == 1 && e.base >= 0x0010_0000 {
                if e.length > best_len {
                    best_base = e.base;
                    best_len = e.length;
                }
            }
        }

        if best_len == 0 {
            return None;
        }

        let start = align_up(best_base, PAGE_SIZE);
        let end = best_base + best_len;

        Some(Self {
            current: start,
            end,
        })
    }

    /// Allocate one 4 KiB physical frame
    pub fn alloc_frame(&mut self) -> Option<u64> {
        if self.current + PAGE_SIZE > self.end {
            return None;
        }

        let frame = self.current;
        self.current += PAGE_SIZE;
        Some(frame)
    }
}

fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}
