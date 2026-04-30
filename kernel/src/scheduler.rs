use crate::task::{Task, TaskState, MAX_TASKS};
use core::arch::{asm, naked_asm};

static mut TASKS: [Option<Task>; MAX_TASKS] = {
    const NONE: Option<Task> = None;
    [NONE; MAX_TASKS]
};
static mut CURRENT: usize = 0;
static mut COUNT: usize = 0;
static mut STARTED: bool = false;

pub fn init() {
    unsafe {
        TASKS[0] = Some(Task::new_idle(0));
        CURRENT = 0;
        COUNT = 1;
        STARTED = true;
    }
    crate::serial::println("Scheduler: initialized (task 0 = idle/main)");
}

pub fn spawn(entry: fn() -> !) {
    unsafe {
        if COUNT >= MAX_TASKS {
            crate::serial::println("Scheduler: max tasks reached");
            return;
        }
        let id = COUNT;
        TASKS[id] = Some(Task::new(id, entry));
        COUNT += 1;
        crate::serial::print("Scheduler: spawned task ");
        crate::serial::print_num(id as u64);
        crate::serial::println("");
    }
}

pub fn exit_current() {
    unsafe {
        asm!("cli", options(nomem, nostack));
        if let Some(ref mut task) = TASKS[CURRENT] {
            task.state = TaskState::Exited;
        }
        asm!("sti", options(nomem, nostack));
    }
    loop { unsafe { asm!("hlt"); } }
}

/// Called from the timer ISR with RSP pointing to:
///   [r11, r10, r9, r8, rdi, rsi, rdx, rcx, rax, iretq_frame...]
/// We save this RSP as the task's stack pointer and switch to another task.
#[inline(never)]
pub fn schedule_from_isr(isr_rsp: u64) -> u64 {
    unsafe {
        if !STARTED || COUNT <= 1 { return isr_rsp; }

        let prev = CURRENT;
        let mut next = (prev + 1) % COUNT;
        let mut found = false;

        for _ in 0..COUNT {
            if let Some(ref task) = TASKS[next] {
                if task.state == TaskState::Ready {
                    found = true;
                    break;
                }
            }
            next = (next + 1) % COUNT;
        }

        if !found { return isr_rsp; }

        // Save current task's ISR stack pointer
        if let Some(ref mut task) = TASKS[prev] {
            task.rsp = isr_rsp;
            if task.state == TaskState::Running {
                task.state = TaskState::Ready;
            }
        }

        // Switch to next task
        if let Some(ref mut task) = TASKS[next] {
            task.state = TaskState::Running;
            CURRENT = next;
            return task.rsp;
        }

        isr_rsp
    }
}
