// size of the physical memory ~words (ints)
const PM_SIZE: usize = 524_288;
const FRAME_SIZE: usize = 512;
const NUM_FRAMES: usize = 1024;
const DISK_BLOCKS: usize = 1024;

// bitmasks
const MASK_9_BITS: u32 = 0x1FF;
const MASK_18_BITS: u32 = 0x3FFFF;
