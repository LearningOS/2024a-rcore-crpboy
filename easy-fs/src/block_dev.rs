use core::any::Any;
/// Trait for block devices
/// which reads and writes data in the unit of blocks
/// easy-fs 的使用者将负责提供抽象方法的实现, 在本crate里不需要进行实现
/// 我们会在os里面对它进行实现
/// 对应的是磁盘里的一个块的读写控制器
pub trait BlockDevice: Send + Sync + Any {
    ///Read data form block to buffer
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    ///Write data from buffer to block
    fn write_block(&self, block_id: usize, buf: &[u8]);
}
