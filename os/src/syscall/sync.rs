//! 在这里我们对于信号量机制和条件变量机制都进行了完整的实现

use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;

/// if sys_enable_deadlock_detect is called
/// then mutex operation should return this value if it detect deadlock
const DEADLOCK_IDENTIFIER: isize = -0xdead;

/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
/// 创建互斥资源
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_deadlock_detector.rebase_resource(id, 1);
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_deadlock_detector.create_resource(1);
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
/// 占用互斥资源
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    if !process_inner
        .mutex_deadlock_detector
        .try_request(tid, mutex_id, 1)
    {
        return DEADLOCK_IDENTIFIER;
    }
    drop(process_inner);
    drop(process);
    mutex.lock();
    current_process()
        .inner_exclusive_access()
        .mutex_deadlock_detector
        .alloc(tid, mutex_id, 1);
    0
}
/// mutex unlock syscall
/// 解除互斥资源占用
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    drop(process_inner);
    drop(process);
    current_process()
        .inner_exclusive_access()
        .mutex_deadlock_detector
        .dealloc(tid, mutex_id, 1);
    mutex.unlock();
    0
}
/// semaphore create syscall
/// 创建信号量资源
pub fn sys_semaphore_create(res_count: usize) -> isize {
    let pid = current_process().pid.0;
    if pid == 15 {
        info!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
        );
    }
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner
            .semaphore_deadlock_detector
            .rebase_resource(id, res_count);
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_deadlock_detector
            .create_resource(res_count);
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let pid = current_process().pid.0;
    if pid == 15 {
        info!(
            "kernel:pid[{}] tid[{}] sys_semaphore_up",
            current_task().unwrap().process.upgrade().unwrap().getpid(),
            current_task()
                .unwrap()
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .tid
        );
    }
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    drop(process_inner);
    current_process()
        .inner_exclusive_access()
        .semaphore_deadlock_detector
        .dealloc(tid, sem_id, 1);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let pid = current_process().pid.0;
    if pid == 15 {
        info!(
            "kernel:pid[{}] tid[{}] sys_semaphore_down",
            current_task().unwrap().process.upgrade().unwrap().getpid(),
            current_task()
                .unwrap()
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .tid
        );
    }
    // let mut ret_value = 0;
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    if !process_inner
        .semaphore_deadlock_detector
        .try_request(tid, sem_id, 1)
    {
        return DEADLOCK_IDENTIFIER;
    }
    drop(process_inner);
    current_task().unwrap().inner_exclusive_access()
        .res.as_mut().unwrap().hang_sem_id = sem_id;
    if sem.down() {
        current_process()
            .inner_exclusive_access()
            .semaphore_deadlock_detector
            .alloc(tid, sem_id, 1);
    }
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
pub fn sys_enable_deadlock_detect(is_enabled: usize) -> isize {
    info!(
        "kernel:pid[{}] tid[{}] sys_enable_deadlock_detect",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    match is_enabled {
        0 => {
            process_inner.mutex_deadlock_detector.disable();
            process_inner.semaphore_deadlock_detector.disable();
            0
        }
        1 => {
            process_inner.mutex_deadlock_detector.enable();
            process_inner.semaphore_deadlock_detector.enable();
            0
        }
        _ => -1,
    }
}
