//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::mm::{MapPermission, VirtPageNum};
use crate::sync::UPSafeCell;
use crate::syscall::TaskInfo;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }

    /// return real task info
    pub fn get_task_info(&self) -> TaskInfo {
        let current = self.current.as_ref().map(Arc::clone).unwrap();
        let inner = current.inner_exclusive_access();
        let status = inner.task_status;
        let task_info = inner.task_info;
        drop(inner);
        TaskInfo {
            status,
            syscall_times: task_info.syscall_times,
            time: task_info.get_time(),
        }
    }

    /// sys_mmap inner call
    /// assume that pages in range are not assigned
    pub fn do_mmap(&self, start: usize, size: usize, perm: MapPermission) -> bool {
        let end = start + size;
        self.current()
            .unwrap()
            .inner_exclusive_access()
            .memory_set
            .insert_framed_area(start.into(), end.into(), perm);
        true
    }

    /// sys_munmap inner call
    /// assume that pages in range are all successfully assigned
    pub fn do_munmap(&self, vpn: VirtPageNum) -> bool {
        self.current()
            .unwrap()
            .inner_exclusive_access()
            .memory_set
            .unmap(vpn);
        true
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

/// check if the virt page num exists in current task's page
pub fn check_vpn_exists(vpn: VirtPageNum) -> bool {
    if let Some(pte) = PROCESSOR
        .exclusive_access()
        .current()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .translate(vpn)
    {
        pte.is_valid()
    } else {
        false
    }
}

/// syscall taskinfo statistic
pub fn task_info_statistic(syscall_id: usize) {
    PROCESSOR
        .exclusive_access()
        .current()
        .unwrap()
        .inner_exclusive_access()
        .task_info
        .task_info_statistic(syscall_id)
}

/// task info call
pub fn get_task_info() -> TaskInfo {
    PROCESSOR.exclusive_access().get_task_info()
}

/// syscall mmap
pub fn do_mmap(start: usize, size: usize, perm: MapPermission) -> bool {
    PROCESSOR.exclusive_access().do_mmap(start, size, perm)
}

/// syscall munmap
pub fn do_munmap(vpn: VirtPageNum) -> bool {
    PROCESSOR.exclusive_access().do_munmap(vpn)
}
