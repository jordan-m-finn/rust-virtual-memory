pub const S_BITS: u32 = 9;
pub const P_BITS: u32 = 9;
pub const W_BITS: u32 = 9;

pub const PAGE_SIZE: usize = 1 << W_BITS;
pub const PT_SIZE: usize = 1 << P_BITS;
pub const MAX_SEGMENTS: usize = 1 << S_BITS;
pub const ST_SIZE: usize = MAX_SEGMENTS * 2;

pub const NUM_FRAMES: usize = 1024;
pub const PM_SIZE: usize = NUM_FRAMES * PAGE_SIZE;
pub const ST_FRAMES: usize = 2;

pub const DISK_BLOCKS: usize = 1024;
pub const BLOCK_SIZE: usize = PAGE_SIZE;

pub const W_MASK: u32 = (1 << W_BITS) - 1;
pub const P_MASK: u32 = (1 << P_BITS) - 1;
pub const PW_MASK: u32 = (1 << (P_BITS + W_BITS)) - 1;

pub const P_SHIFT: u32 = W_BITS;
pub const S_SHIFT: u32 = P_BITS + W_BITS;

pub const INVALID_ADDRESS: i32 = -1;
