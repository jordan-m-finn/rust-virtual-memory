use crate::constants::*;

pub struct VMManager {
    pm: Vec<i32>, // 524,288 words

    // disk for paging ~1024 blocks x 512 words
    disk: Vec<i32>,
    free_frames: Vec<usize>,
    demand_paging: bool, // flag
}

impl VMManager {
    pub fn new(demand_paging: bool) -> Self {
        VMManager {
            pm: vec![0i32; PM_SIZE],
            disk: vec![0i32; DISK_BLOCKS * FRAME_SIZE],
            free_frames: Vec::new(),
            demand_paging,
        }
    }
}
