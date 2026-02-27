use crate::constants::*;

pub struct PhysicalMemory {
    data: Box<[i32; PM_SIZE]>,
}

impl PhysicalMemory {
    pub fn new() -> Self {
        let data = vec![0i32; PM_SIZE].into_boxed_slice();
        let data: Box<[i32; PM_SIZE]> = data.try_into().unwrap();
        PhysicalMemory { data }
    }

    #[inline]
    pub fn read(&self, address: usize) -> i32 {
        self.data[address]
    }

    #[inline]
    pub fn write(&mut self, address: usize, value: i32) {
        self.data[address] = value;
    }

    #[inline]
    pub fn get_segment_size(&self, segment: u32) -> i32 {
        self.data[2 * segment as usize]
    }

    #[inline]
    pub fn get_segment_pt_location(&self, segment: u32) -> i32 {
        self.data[2 * segment as usize + 1]
    }

    pub fn set_segment_entry(&mut self, segment: u32, size: i32, pt_location: i32) {
        let base = 2 * segment as usize;
        self.data[base] = size;
        self.data[base + 1] = pt_location;
    }

    #[inline]
    pub fn get_page_frame(&self, pt_frame: i32, page: u32) -> i32 {
        let pt_base = pt_frame as usize * PAGE_SIZE;
        self.data[pt_base + page as usize]
    }

    pub fn set_page_entry(&mut self, pt_frame: i32, page: u32, frame_location: i32) {
        let pt_base = pt_frame as usize * PAGE_SIZE;
        self.data[pt_base + page as usize] = frame_location;
    }

    #[inline]
    pub fn frame_to_address(frame: i32) -> usize {
        frame as usize * PAGE_SIZE
    }
}

impl Default for PhysicalMemory {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Disk {
    data: Box<[[i32; BLOCK_SIZE]; DISK_BLOCKS]>,
}

impl Disk {
    pub fn new() -> Self {
        let data = vec![[0i32; BLOCK_SIZE]; DISK_BLOCKS].into_boxed_slice();
        let data: Box<[[i32; BLOCK_SIZE]; DISK_BLOCKS]> = data.try_into().unwrap();
        Disk { data }
    }

    #[inline]
    pub fn read(&self, block: usize, offset: usize) -> i32 {
        self.data[block][offset]
    }

    #[inline]
    pub fn write(&mut self, block: usize, offset: usize, value: i32) {
        self.data[block][offset] = value;
    }

    pub fn read_block(&self, block: usize, pm: &mut PhysicalMemory, pm_start: usize) {
        for i in 0..BLOCK_SIZE {
            pm.write(pm_start + i, self.data[block][i]);
        }
    }

    pub fn load_pt_from_disk(&self, disk_block: usize, frame: u32, pm: &mut PhysicalMemory) {
        let pm_start = PhysicalMemory::frame_to_address(frame as i32);
        self.read_block(disk_block, pm, pm_start);
    }

    pub fn load_page_from_disk(&self, disk_block: usize, frame: u32, pm: &mut PhysicalMemory) {
        let pm_start = PhysicalMemory::frame_to_address(frame as i32);
        self.read_block(disk_block, pm, pm_start);
    }
}

impl Default for Disk {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FreeFrameList {
    free_frames: Vec<u32>,
}

impl FreeFrameList {
    pub fn new() -> Self {
        let free_frames: Vec<u32> = (ST_FRAMES as u32..NUM_FRAMES as u32).rev().collect();
        FreeFrameList { free_frames }
    }

    pub fn mark_occupied(&mut self, frame: u32) {
        if let Some(pos) = self.free_frames.iter().position(|&f| f == frame) {
            self.free_frames.remove(pos);
        }
    }

    pub fn allocate(&mut self) -> Option<u32> {
        self.free_frames.pop()
    }
}

impl Default for FreeFrameList {
    fn default() -> Self {
        Self::new()
    }
}
