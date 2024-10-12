//! Process management syscalls
use crate::{
    config::PAGE_SIZE,
    mm::{translated_byte_buffer, MapPermission, VPNRangeOuter, VirtAddr, VirtPageNum},
    task::{
        change_program_brk, check_vpn_exists, current_user_token, do_mmap, do_munmap,
        exit_current_and_run_next, suspend_current_and_run_next, TaskInfo, TASK_MANAGER,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

fn copy_from_kernel_space<T>(tar_ptr: *mut T, res: &T) {
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
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let time_us = get_time_us();
    let res = TimeVal {
        sec: time_us / 1_000_000,
        usec: time_us % 1_000_000,
    };
    copy_from_kernel_space(_ts, &res);
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    let res = TASK_MANAGER.get_task_info();
    // info!("task_req: {:?}\nday: {:?}", res, res.syscall_times[169]);
    copy_from_kernel_space(_ti, &res);
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, _port: usize) -> isize {
    info!("mmap called");

    // basic check
    if start % PAGE_SIZE != 0 || (_port & !0x7) != 0 || _port & 0x7 == 0 {
        return -1;
    }

    // check if pages already exists
    let end = start + len;
    let s_vpn: VirtPageNum = VirtAddr::from(start).floor().into();
    let e_vpn: VirtPageNum = VirtAddr::from(end).ceil().into();
    for cur_vpn in VPNRangeOuter::new(s_vpn, e_vpn) {
        info!("range: {start} to {end}, cur_vpn: {:?}", cur_vpn);
        if check_vpn_exists(cur_vpn) {
            return -1;
        }
    }

    // insert page map
    let perm = MapPermission::from_bits_truncate((_port << 1) as u8) | MapPermission::U;
    // info!("{:?} {:?}", _port, perm);
    if !do_mmap(start, len, perm) {
        return -1;
    }
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    info!("mUNmap called");

    // basic check
    if start % PAGE_SIZE != 0 {
        return -1;
    }

    // check if pages already exists
    let end = start + len;
    let s_vpn: VirtPageNum = VirtAddr::from(start).floor().into();
    let e_vpn: VirtPageNum = VirtAddr::from(end).ceil().into();
    for cur_vpn in VPNRangeOuter::new(s_vpn, e_vpn) {
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
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
