use crate::E820Entry;

const PAGE_SIZE: u64 = 4096;

pub struct PhysicalMemoryManager {
    current: u64,
    end: u64,
}

impl PhysicalMemoryManager {
    pub fn new(e820: &[E820Entry]) -> Option<Self> {
        let mut best_base = 0;
        let mut best_len = 0;

        for e in e820 {
            if e.kind == 1 && e.base >= 0x0010_0000 {
                if e.length > best_len {
                    best_base = e.base;
                    best_len = e.length;
                }
            }
        }
        if best_len == 0 { return None; }

        // Skip first 2MB to avoid kernel image overlap
        let kernel_end = 0x0020_0000;
        if best_base < kernel_end {
            let diff = kernel_end - best_base;
            if best_len <= diff { return None; }
            best_base = kernel_end;
            best_len -= diff;
        }

        let start = align_up(best_base, PAGE_SIZE);
        let end = best_base + best_len;
        Some(Self { current: start, end })
    }

    pub fn get_state(&self) -> (u64, u64) { (self.current, self.end) }
    pub fn advance_to(&mut self, new_current: u64) { self.current = new_current; }

    pub fn alloc_frame(&mut self) -> Option<u64> {
        if self.current + PAGE_SIZE > self.end { return None; }
        let frame = self.current;
        self.current += PAGE_SIZE;
        Some(frame)
    }
}

fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}
