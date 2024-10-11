//! Types related to task management

use crate::{config::MAX_SYSCALL_NUM, timer::get_time_ms};

use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// The task info
    pub task_info: TaskInfoInner,
}

/// The status of a task
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}

/// task info in response
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct TaskInfoInner {
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// last recorded start time of the task
    pub start_time: usize,
    /// mark if it is launched
    pub launch_flag: bool,
}

impl TaskInfoInner {
    pub fn new_bare() -> Self {
        Self {
            syscall_times: [0; MAX_SYSCALL_NUM],
            start_time: 0,
            launch_flag: false,
        }
    }
    pub fn get_time(&self) -> usize {
        if !self.launch_flag {
            0
        } else {
            get_time_ms() - self.start_time
        }
    }
}
