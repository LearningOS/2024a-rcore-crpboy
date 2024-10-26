//! use deadlock detector to detect deadlock
//! only update its status when detect is enabled

use alloc::vec;
use alloc::vec::Vec;

/// overall dead lock detector
#[derive(Debug)]
pub struct DeadlockDetector {
    /// 可利用资源向量 Available
    /// 每个元素代表可利用的某一类资源的数目
    /// 其初值是该类资源的全部可用数目
    /// 其值随该类资源的分配和回收而动态地改变
    available_resources: Vec<usize>,

    /// 分配矩阵 Allocation
    /// 表示每类资源已分配给每个线程的资源数
    /// Allocation[i,j] = g，则表示线程 i 当前己分得第 j 类资源的数量为 g
    task_allocation: Vec<Vec<usize>>,

    /// 需求矩阵 Need
    /// 表示每个线程还需要的各类资源数量
    /// Need[i,j] = d，则表示线程 i 还需要第 j 类资源的数量为 d
    task_need: Vec<Vec<usize>>,

    /// 标记是否已经打开了死锁检测
    enabled: bool,
}

impl DeadlockDetector {
    pub fn new() -> Self {
        // task 0 (main process) already exists
        Self {
            available_resources: Vec::new(),
            task_allocation: vec![Vec::new(); 1],
            task_need: vec![Vec::new(); 1],
            enabled: false,
        }
    }

    /// get length of resource, aka m in this matrix
    fn resource_len(&self) -> usize {
        self.available_resources.len()
    }
    /// get length of task, aka n in this matrix
    fn task_len(&self) -> usize {
        self.task_allocation.len()
    }

    /// enable impl
    pub fn enable(&mut self) {
        info!("deadlock detector enabled");
        self.enabled = true;
    }
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    pub fn is_enable(&self) -> bool {
        self.enabled
    }

    /// create resource / task
    pub fn create_resource(&mut self, res_count: usize) {
        if !self.is_enable() {
            return;
        }

        info!(
            "create resource: rid[{}] num[{}]",
            self.available_resources.len(),
            res_count,
        );
        info!("before: {:?}", self);

        self.available_resources.push(res_count);
        for it in self.task_allocation.iter_mut() {
            it.push(0);
        }
        for it in self.task_need.iter_mut() {
            it.push(0);
        }

        info!("after: {:?}", self);
    }
    pub fn rebase_resource(&mut self, id: usize, res_count: usize) {
        if !self.is_enable() {
            return;
        }
        info!("rebase resource: rid[{}] num[{}]", id, res_count);
        info!("before: {:?}", self);
        assert!(id < self.resource_len());
        self.available_resources[id] = res_count;
        for it in self.task_allocation.iter_mut() {
            it[id] = 0;
        }
        for it in self.task_need.iter_mut() {
            it[id] = 0;
        }
        info!("after: {:?}", self);
    }
    pub fn create_task(&mut self, tid: usize) {
        if !self.is_enable() {
            return;
        }
        info!("create task: tid[{}]", tid);
        info!("before: {:?}", self);
        if tid >= self.task_len() {
            self.task_allocation.push(vec![0; self.resource_len()]);
            self.task_need.push(vec![0; self.resource_len()]);
            assert!(tid < self.task_len());
        } else {
            self.task_allocation[tid] = vec![0; self.resource_len()];
            self.task_need[tid] = vec![0; self.resource_len()];
        }
        info!("after: {:?}", self);
    }

    /// alloc resource
    pub fn alloc(&mut self, tid: usize, rid: usize, num: usize) {
        if !self.is_enable() {
            return;
        }
        info!("alloc: tid[{}] rid[{}] num[{}]", tid, rid, num);
        info!("before: {:?}", self);
        assert!(tid < self.task_len());
        assert!(rid < self.resource_len());
        assert!(self.available_resources[rid] >= num);
        self.available_resources[rid] -= num;
        self.task_allocation[tid][rid] += num;
        self.task_need[tid][rid] -= num;
        info!("after: {:?}", self);
    }

    /// dealloc resource
    pub fn dealloc(&mut self, tid: usize, rid: usize, num: usize) {
        if !self.is_enable() {
            return;
        }
        info!("dealloc: tid[{}] rid[{}] num[{}]", tid, rid, num);
        info!("before: {:?}", self);
        assert!(tid < self.task_len());
        assert!(rid < self.resource_len());
        self.available_resources[rid] += num;
        self.task_allocation[tid][rid] -= num;
        // self.task_need[tid][rid] += num;
        info!("after: {:?}", self);
    }

    /// returns true when detect deadlock
    fn is_deadlock(&self) -> bool {
        if !self.is_enable() {
            return false;
        }
        info!("deadlock check: self[{:?}]", self);
        let mut work = self.available_resources.clone();
        let mut finish = vec![false; self.task_len()];
        loop {
            let mut flag = false;
            info!("doing check: finish[{:?}], work[{:?}]", finish, work);
            for i in 0..self.task_len() {
                if !finish[i] && self.available_resources.iter().enumerate()
                    .all(|(j, number)| self.task_need[i][j] <= work[j]) {
                    flag = true;
                    finish[i] = true;
                    for j in 0..self.resource_len() {
                        info!("task[{}] can finish with allocation[{}]", i, j);
                        work[j] += self.task_allocation[i][j];
                    }
                    break;
                }
            }
            if !flag {
                break;
            }
        }
        for i in finish.iter() {
            // task isn't finished -> deadlock exists
            if !i {
                return true;
            }
        }
        return false;
    }

    /// lock resource but not alloc
    /// task index, resource index, num
    pub fn try_request(&mut self, tid: usize, rid: usize, num: usize) -> bool {
        if !self.is_enable() {
            return true;
        }
        info!("try_request: tid[{}] rid[{}] num[{}]", tid, rid, num);
        assert!(tid < self.task_len());
        assert!(rid < self.resource_len());
        self.task_need[tid][rid] += num;
        let res = self.is_deadlock();
        if res {
            info!("DEADLOCK DETECTED!!!\n{:?}\n", self);
        }
        !res
    }
}
