use super::{get_block_cache, BlockDevice, BLOCK_SZ};
use alloc::sync::Arc;
/// A bitmap block
type BitmapBlock = [u64; 64];
/// Number of bits in a block
const BLOCK_BITS: usize = BLOCK_SZ * 8;
/// A bitmap
/// bitmap其实就是bitset形式的bool数组, 维护若干块的占用情况
/// 它是一个由若干个4096bit的块组成的大bitset
/// 它维护了由start_id开始, 共计blocks个的块的占用情况
pub struct Bitmap {
    start_block_id: usize,
    blocks: usize,
}

/// Decompose bits into (block_pos, bits64_pos, inner_pos)
/// 找到第bit个比特位的在块上的真实位置
/// 它在block_pos个块, 块内位置为第bit64_pos个, usize内位置为第inner_pos位
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit %= BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}

impl Bitmap {
    /// A new bitmap from start block id and number of blocks
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }
    /// Allocate a new block from a block device
    /// 找到bitmap中第一个为0的位, 将其置为1并返回对应id
    /// 优化: 我觉得这样很蠢, 一个bitmap你去遍历单个block内的所有数, 完全没有必要
    /// 可以对于每个block, 额外维护一个block_bit, 表示它内部每个数字是否已满
    /// 刚好64 * 64 = 4096, 我们可以很好地使用一个block_bit来一级索引它的low_zerobit
    /// 相当于一个两层的树(类似线段树上二分的想法), 先去查询block_bit, 定位到哪一位为空
    /// 然后再去查询对应元素, 通过位运算找到low_zerobit即可
    /// 对于更高层也可以这么维护, 形成树形查找结构, 复杂度为log64(bit_count)
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        for block_id in 0..self.blocks {
            // 向manager申请访问bitmap对应的当前block
            let pos = get_block_cache(
                block_id + self.start_block_id as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                if let Some((bits64_pos, inner_pos)) = bitmap_block
                    .iter()
                    .enumerate()
                    .find(|(_, bits64)| **bits64 != u64::MAX)
                    .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                {
                    // modify cache
                    // u64::MAX也就是所有位都为1, 也即被占满了
                    // 我们的任务是找到第一个不为全满的bit数, 所以判断它不为MAX
                    // trailing ones就是尾部的0个数, 将它作为inner_pos, 也就是找到low_zero_bit作为插入的节点
                    bitmap_block[bits64_pos] |= 1u64 << inner_pos;
                    Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                } else {
                    None
                }
            });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }
    /// Deallocate a block
    /// 将bit位置为0
    /// 要求该bit位在此之前需要为1
    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_pos + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
                bitmap_block[bits64_pos] -= 1u64 << inner_pos;
            });
    }
    /// Get the max number of allocatable blocks
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}
