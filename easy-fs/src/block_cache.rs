use super::{BlockDevice, BLOCK_SZ};
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
use spin::Mutex;
/// Cached block inside memory
/// 一个block cache对应着一个磁盘block在内存里的缓存
pub struct BlockCache {
    /// cached block data
    /// 用于存放数据
    cache: [u8; BLOCK_SZ],
    /// underlying block id
    /// 磁盘编号
    block_id: usize,
    /// underlying block device
    /// 通过trait进行泛型描述, 只要是实现了read/write函数的设备都可以进行交互
    block_device: Arc<dyn BlockDevice>,
    /// whether the block is dirty
    /// 其实就是cache的dirt位
    modified: bool,
}

impl BlockCache {
    /// Load a new BlockCache from disk.
    /// 对于目标磁盘创建一个cache
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut cache = [0u8; BLOCK_SZ];
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }
    /// Get the address of an offset inside the cached block data
    /// 返回地址指针的原始数据
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    }

    /// 返回offset对应的引用
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    /// 返回offset对应的可变引用
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }

    /// 读取数据, 并经过闭包处理
    /// 为什么要这么搞, 我直接用get_ref/mut不行吗
    /// 其实是因为后面要通过多级索引进行访问, 多级索引访问的时候会出现需要映射的情况
    /// 这里的read就是用于解决多级映射而创建的
    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    /// 使用闭包修改数据
    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    /// 向磁盘同步数据, 如果dirt则写回数据
    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }
}

/// drop的时候检查dirt位 检测是否写回
impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}
/// Use a block cache of 16 blocks
const BLOCK_CACHE_SIZE: usize = 16;

/// 把生命周期绑定在一个queue上
/// 使用arc+mutex进行共享引用互斥访问
pub struct BlockCacheManager {
    queue: VecDeque<(usize, Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// 尝试从block_device的id下标获取一个cache
    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        // 找到了就返回
        // q: 这里似乎不能保证对应的device是同一个?
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            Arc::clone(&pair.1)
        } else {
            // substitute
            // 首先判断队列是否已满
            // 如果满了进行cache的替换, 这样才可以顺利插入一个新的cache
            if self.queue.len() == BLOCK_CACHE_SIZE {
                // from front to tail
                // 找到第一个strong_count=1的cache
                // count=1的意思就是, 除了self.queue以外没有任何地方对它进行引用
                // 也就是说它是空闲的, 可以被写回到磁盘当中
                // q: 这里的替换策略是带有优先级的
                // 是不是改成类似仲裁器的轮询策略会好一点?
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    // 如果发现连写回都不行, 那就只好panic了, 不过因为是单核处理器, 大概率不会发生吧?
                    // 其实我觉得这边可以进行阻塞等待, 加一个loop进行访问我觉得也是可以的, 多核情况下说不定能遇到
                    panic!("Run out of BlockCache!");
                }
            }
            // load block into mem and push back
            // 插入一个新的cache
            let block_cache = Arc::new(Mutex::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            self.queue.push_back((block_id, Arc::clone(&block_cache)));
            block_cache
        }
    }
}

lazy_static! {
    /// The global block cache manager
    /// 为什么要用Mutex 因为使用的是同一个硬盘, 属于临界资源, 必须保证只能被一个进程访问
    pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
        Mutex::new(BlockCacheManager::new());
}
/// Get the block cache corresponding to the given block id and block device
pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}
/// Sync all block cache to block device
/// cache全部写回磁盘
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache) in manager.queue.iter() {
        cache.lock().sync();
    }
}
