//! Conditian variable

use crate::sync::{Mutex, UPSafeCell};
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// Condition variable structure
/// 条件变量
pub struct Condvar {
    /// Condition variable inner
    pub inner: UPSafeCell<CondvarInner>,
}

pub struct CondvarInner {
    /// 对于每个条件变量的等待队列, 都保存正在等待这个条件变量成立的对应任务
    /// 形成一个wait_queue
    /// 这样方便进行唤醒
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Condvar {
    /// Create a new condition variable
    pub fn new() -> Self {
        trace!("kernel: Condvar::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(CondvarInner {
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// Signal a task waiting on the condition variable
    /// signal操作, 用于唤醒一个等待在条件变量上的任务
    pub fn signal(&self) {
        let mut inner = self.inner.exclusive_access();
        if let Some(task) = inner.wait_queue.pop_front() {
            wakeup_task(task);
        }
    }

    /// blocking current task, let it wait on the condition variable
    /// wait操作, 在当前任务阻塞在这一条件变量上的同时, 插入对应条件变量的等待队列
    pub fn wait(&self, mutex: Arc<dyn Mutex>) {
        trace!("kernel: Condvar::wait_with_mutex");
        mutex.unlock();
        let mut inner = self.inner.exclusive_access();
        inner.wait_queue.push_back(current_task().unwrap());
        drop(inner);
        block_current_and_run_next();
        mutex.lock();
    }
}
