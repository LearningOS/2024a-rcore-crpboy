//! Process management syscalls
//!
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    fs::{open_file, OpenFlags},
    mm::{
        translated_byte_buffer, translated_refmut, translated_str, MapPermission, VPNRange,
        VirtAddr, VirtPageNum,
    },
    task::{
        add_task, check_vpn_exists, current_task, current_user_token, do_mmap, do_munmap,
        exit_current_and_run_next, get_task_info, set_new_prio, suspend_current_and_run_next,
        TaskStatus,
    },
    timer::get_time_us,
};
use alloc::sync::Arc;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// when using ptr in user space, should copy it from kernel space
/// this func will be called in any syscall with ptr returned
pub fn copy_from_kernel_space<T>(tar_ptr: *mut T, res: &T) {
    let tar = translated_byte_buffer(
        current_user_token(),
        tar_ptr as *const u8,
        core::mem::size_of::<T>(),
    );
    let mut res_ptr = res as *const T;
    for dst in tar.into_iter() {
        unsafe {
            dst.copy_from_slice(core::slice::from_raw_parts(res_ptr as *const u8, dst.len()));
            res_ptr = res_ptr.add(dst.len());
        }
    }
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel:pid[{}] sys_get_time", current_task().unwrap().pid.0);
    let time_us = get_time_us();
    let res = TimeVal {
        sec: time_us / 1_000_000,
        usec: time_us % 1_000_000,
    };
    copy_from_kernel_space(ts, &res);
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!(
        "kernel:pid[{}] sys_task_info",
        current_task().unwrap().pid.0
    );
    let res = get_task_info();
    copy_from_kernel_space(ti, &res);
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, _port: usize) -> isize {
    trace!("kernel:pid[{}] sys_mmap", current_task().unwrap().pid.0);
    // basic check
    if start % PAGE_SIZE != 0 || (_port & !0x7) != 0 || _port & 0x7 == 0 {
        return -1;
    }
    // check if pages already exists
    let end = start + len;
    let s_vpn: VirtPageNum = VirtAddr::from(start).floor().into();
    let e_vpn: VirtPageNum = VirtAddr::from(end).ceil().into();
    for cur_vpn in VPNRange::new(s_vpn, e_vpn) {
        info!("range: {start} to {end}, cur_vpn: {:?}", cur_vpn);
        if check_vpn_exists(cur_vpn) {
            return -1;
        }
    }
    // insert page map
    let perm = MapPermission::from_bits_truncate((_port << 1) as u8) | MapPermission::U;
    if !do_mmap(start, len, perm) {
        return -1;
    }
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_munmap", current_task().unwrap().pid.0);
    // basic check
    if start % PAGE_SIZE != 0 {
        return -1;
    }
    // check if pages already exists
    let end = start + len;
    let s_vpn: VirtPageNum = VirtAddr::from(start).floor().into();
    let e_vpn: VirtPageNum = VirtAddr::from(end).ceil().into();
    for cur_vpn in VPNRange::new(s_vpn, e_vpn) {
        info!("range: {:X} to {:X}, cur_vpn: {:?}", start, end, cur_vpn);
        if !check_vpn_exists(cur_vpn) {
            return -1;
        } else {
            do_munmap(cur_vpn);
        }
    }
    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_spawn", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let data = app_inode.read_all();
        let new_task = current_task().unwrap().spawn(data.as_slice());
        let new_pid = new_task.pid.0;
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        trap_cx.x[10] = 0;
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
    if prio >= 2 {
        set_new_prio(prio);
        prio
    } else {
        -1
    }
}
