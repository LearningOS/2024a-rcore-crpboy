//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: Vec<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: Vec::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        if self.ready_queue.is_empty() {
            return None;
        }
        let mut min_id = 0;
        let mut min_value = core::isize::MAX;
        for (id, it) in self.ready_queue.iter().enumerate() {
            let value = it.get_stride();
            if value < min_value {
                min_value = value;
                min_id = id;
            }
        }

        // debug
        // {
        //     static mut COUNTER: usize = 0;
        //     unsafe {
        //         COUNTER += 1;
        //     }
        //     if unsafe { COUNTER % 100 == 0 } {
        //         info!("priority queue info:");
        //         for now in self.ready_queue.iter() {
        //             let info = &now.inner_exclusive_access().stride_block;
        //             info!(
        //                 "prio {}: stride is {}, pass is {}",
        //                 info.prio, info.stride, info.pass
        //             );
        //         }
        //         info!("poped value: {} in {}", min_value, min_id);
        //     }
        // }

        Some(self.ready_queue.remove(min_id))
    }
}

/// impl this for binary heap
/// discard
impl PartialEq for TaskControlBlock {
    fn eq(&self, other: &Self) -> bool {
        self.get_stride() == other.get_stride()
    }
}
impl Eq for TaskControlBlock {}
impl PartialOrd for TaskControlBlock {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TaskControlBlock {
    /// order reversed
    /// shoule let the smallest strided task run
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.get_stride().cmp(&self.get_stride())
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
