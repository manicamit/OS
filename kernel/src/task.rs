pub const MAX_TASKS: usize = 64;
pub const TASK_STACK_SIZE: usize = 8192;

#[derive(Clone, Copy, PartialEq)]
pub enum TaskState {
    Ready,
    Running,
    Exited,
}

/// Layout must match the ISR push order in irq0_timer_handler:
/// push rax, rcx, rdx, rbx, rbp, rsi, rdi, r8..r15
/// Then on the stack below that: the iretq frame (RIP, CS, RFLAGS, RSP, SS)
#[repr(C)]
pub struct IsrFrame {
    pub r15: u64, pub r14: u64, pub r13: u64, pub r12: u64,
    pub r11: u64, pub r10: u64, pub r9: u64, pub r8: u64,
    pub rdi: u64, pub rsi: u64, pub rbp: u64,
    pub rbx: u64, pub rdx: u64, pub rcx: u64, pub rax: u64,
    // iretq frame
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

pub struct Task {
    pub id: usize,
    pub state: TaskState,
    pub rsp: u64,
    pub stack_bottom: u64,
}

impl Task {
    pub fn new_idle(id: usize) -> Self {
        Task { id, state: TaskState::Running, rsp: 0, stack_bottom: 0 }
    }

    pub fn new(id: usize, entry: fn() -> !) -> Self {
        let stack = unsafe {
            alloc::alloc::alloc(
                alloc::alloc::Layout::from_size_align(TASK_STACK_SIZE, 16).unwrap()
            )
        };
        if stack.is_null() {
            panic!("Failed to allocate task stack");
        }
        let stack_bottom = stack as u64;
        let stack_top = stack_bottom + TASK_STACK_SIZE as u64;

        // Build a fake ISR frame so the first timer interrupt "returns" into entry
        let frame_size = core::mem::size_of::<IsrFrame>() as u64;
        let frame_addr = stack_top - frame_size;
        let frame = frame_addr as *mut IsrFrame;

        unsafe {
            (*frame).r15 = 0; (*frame).r14 = 0; (*frame).r13 = 0; (*frame).r12 = 0;
            (*frame).r11 = 0; (*frame).r10 = 0; (*frame).r9 = 0; (*frame).r8 = 0;
            (*frame).rdi = 0; (*frame).rsi = 0; (*frame).rbp = 0;
            (*frame).rbx = 0; (*frame).rdx = 0; (*frame).rcx = 0; (*frame).rax = 0;

            (*frame).rip = entry as u64;
            (*frame).cs = 0x18;        // kernel code segment
            (*frame).rflags = 0x202;   // IF=1 (interrupts enabled)
            (*frame).rsp = stack_top;  // task's own stack top
            (*frame).ss = 0x20;        // kernel data segment
        }

        Task {
            id,
            state: TaskState::Ready,
            rsp: frame_addr,
            stack_bottom,
        }
    }
}

use alloc;
