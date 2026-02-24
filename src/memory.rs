use crate::constants::*;

pub struct PhysicalMemory {
    data: Box<[i32; PM_SIZE]>,
}

impl PhysicalMemory {
    /// Create a new physical memory initialized to all zeros
    pub fn new() -> Self {
        // Use vec! to allocate on heap, then convert to boxed array
        let data = vec![0i32; PM_SIZE].into_boxed_slice();
        let data: Box<[i32; PM_SIZE]> = data.try_into().unwrap();
        PhysicalMemory { data }
    }

    /// Read a word from physical memory
    #[inline]
    pub fn read(&self, address: usize) -> i32 {
        self.data[address]
    }

    /// Write a word to physical memory
    #[inline]
    pub fn write(&mut self, address: usize, value: i32) {
        self.data[address] = value;
    }

    /// Get the size of a segment from the Segment Table
    #[inline]
    pub fn get_segment_size(&self, segment: u32) -> i32 {
        self.data[2 * segment as usize]
    }

    /// Get the PT frame/block for a segment from the Segment Table
    #[inline]
    pub fn get_segment_pt_location(&self, segment: u32) -> i32 {
        self.data[2 * segment as usize + 1]
    }

    /// Set a Segment Table entry
    pub fn set_segment_entry(&mut self, segment: u32, size: i32, pt_location: i32) {
        let base = 2 * segment as usize;
        self.data[base] = size;
        self.data[base + 1] = pt_location;
    }

    /// Get a Page Table entry
    #[inline]
    pub fn get_page_frame(&self, pt_frame: i32, page: u32) -> i32 {
        let pt_base = pt_frame as usize * PAGE_SIZE;
        self.data[pt_base + page as usize]
    }

    /// Set a Page Table entry
    pub fn set_page_entry(&mut self, pt_frame: i32, page: u32, frame_location: i32) {
        let pt_base = pt_frame as usize * PAGE_SIZE;
        self.data[pt_base + page as usize] = frame_location;
    }

    /// Calculate the starting address of a frame
    #[inline]
    pub fn frame_to_address(frame: i32) -> usize {
        frame as usize * PAGE_SIZE
    }

    /// Get direct access to the underlying data (for bulk operations)
    pub fn data(&self) -> &[i32; PM_SIZE] {
        &self.data
    }

    /// Get mutable access to the underlying data (for bulk operations like read_block)
    pub fn data_mut(&mut self) -> &mut [i32; PM_SIZE] {
        &mut self.data
    }
}

impl Default for PhysicalMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Paging Disk - simulates secondary storage for demand paging
pub struct Disk {
    /// 2D array: D[block][offset] where block is 0-1023 and offset is 0-511
    data: Box<[[i32; BLOCK_SIZE]; DISK_BLOCKS]>,
}

impl Disk {
    /// Create a new disk initialized to all zeros
    pub fn new() -> Self {
        // Create a zeroed 2D array on the heap
        let data = vec![[0i32; BLOCK_SIZE]; DISK_BLOCKS].into_boxed_slice();
        let data: Box<[[i32; BLOCK_SIZE]; DISK_BLOCKS]> = data.try_into().unwrap();
        Disk { data }
    }

    /// Read a word from disk
    #[inline]
    pub fn read(&self, block: usize, offset: usize) -> i32 {
        self.data[block][offset]
    }

    /// Write a word to disk
    #[inline]
    pub fn write(&mut self, block: usize, offset: usize, value: i32) {
        self.data[block][offset] = value;
    }

    /// Read an entire block from disk into physical memory
    pub fn read_block(&self, block: usize, pm: &mut PhysicalMemory, pm_start: usize) {
        for i in 0..BLOCK_SIZE {
            pm.write(pm_start + i, self.data[block][i]);
        }
    }

    /// Get direct access to a disk block (for initialization)
    pub fn block_mut(&mut self, block: usize) -> &mut [i32; BLOCK_SIZE] {
        &mut self.data[block]
    }
}

impl Default for Disk {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks which frames are available for allocation
pub struct FreeFrameList {
    // TODO: implement free frame tracking
    _placeholder: (),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pm_initialization() {
        let pm = PhysicalMemory::new();
        // All memory should be zeroed
        assert_eq!(pm.read(0), 0);
        assert_eq!(pm.read(PM_SIZE - 1), 0);
    }

    #[test]
    fn test_pm_read_write() {
        let mut pm = PhysicalMemory::new();
        pm.write(100, 42);
        assert_eq!(pm.read(100), 42);

        pm.write(100, -7); // Negative values for disk block references
        assert_eq!(pm.read(100), -7);
    }

    #[test]
    fn test_segment_table_operations() {
        let mut pm = PhysicalMemory::new();

        // Set up segment 6 with size 3000, PT in frame 4
        pm.set_segment_entry(6, 3000, 4);

        assert_eq!(pm.get_segment_size(6), 3000);
        assert_eq!(pm.get_segment_pt_location(6), 4);

        // Verify raw memory locations
        assert_eq!(pm.read(12), 3000); // PM[2*6] = size
        assert_eq!(pm.read(13), 4);    // PM[2*6+1] = frame
    }

    #[test]
    fn test_page_table_operations() {
        let mut pm = PhysicalMemory::new();

        // PT is in frame 4, set page 5 to be in frame 9
        pm.set_page_entry(4, 5, 9);

        assert_eq!(pm.get_page_frame(4, 5), 9);

        // Verify raw memory location: frame 4 starts at 4*512=2048
        assert_eq!(pm.read(2048 + 5), 9);
    }

    #[test]
    fn test_frame_to_address() {
        assert_eq!(PhysicalMemory::frame_to_address(0), 0);
        assert_eq!(PhysicalMemory::frame_to_address(1), 512);
        assert_eq!(PhysicalMemory::frame_to_address(4), 2048);
        assert_eq!(PhysicalMemory::frame_to_address(10), 5120);
    }

    #[test]
    fn test_disk_initialization() {
        let disk = Disk::new();
        assert_eq!(disk.read(0, 0), 0);
        assert_eq!(disk.read(DISK_BLOCKS - 1, BLOCK_SIZE - 1), 0);
    }

    #[test]
    fn test_disk_read_write() {
        let mut disk = Disk::new();
        disk.write(7, 0, 13);  // Block 7, offset 0, value 13
        disk.write(7, 1, -25); // Block 7, offset 1, value -25

        assert_eq!(disk.read(7, 0), 13);
        assert_eq!(disk.read(7, 1), -25);
    }

    #[test]
    fn test_read_block() {
        let mut disk = Disk::new();
        let mut pm = PhysicalMemory::new();

        // Set up disk block 7 with some PT data
        disk.write(7, 0, 13);  // Page 0 -> frame 13
        disk.write(7, 1, -25); // Page 1 -> disk block 25

        // Read block 7 into frame 5 (address 5*512 = 2560)
        disk.read_block(7, &mut pm, 2560);

        assert_eq!(pm.read(2560), 13);
        assert_eq!(pm.read(2561), -25);
    }

    #[test]
    fn test_demand_paging_scenario() {
        // Simulate the example from the spec:
        // Line 1: 8 4000 3   9 5000 -7
        // Line 2: 8 0 10   8 1 -20   9 0 13   9 1 -25

        let mut pm = PhysicalMemory::new();
        let mut disk = Disk::new();

        // Set up ST entries
        pm.set_segment_entry(8, 4000, 3);   // Segment 8: size=4000, PT in frame 3
        pm.set_segment_entry(9, 5000, -7);  // Segment 9: size=5000, PT in disk block 7

        // Set up PT for segment 8 (in frame 3)
        pm.set_page_entry(3, 0, 10);   // Page 0 -> frame 10
        pm.set_page_entry(3, 1, -20);  // Page 1 -> disk block 20

        // Set up PT for segment 9 (on disk block 7)
        disk.write(7, 0, 13);   // Page 0 -> frame 13
        disk.write(7, 1, -25);  // Page 1 -> disk block 25

        // Verify segment 8
        assert_eq!(pm.get_segment_size(8), 4000);
        assert_eq!(pm.get_segment_pt_location(8), 3);
        assert_eq!(pm.get_page_frame(3, 0), 10);
        assert_eq!(pm.get_page_frame(3, 1), -20);

        // Verify segment 9
        assert_eq!(pm.get_segment_size(9), 5000);
        assert_eq!(pm.get_segment_pt_location(9), -7); // On disk!
        assert_eq!(disk.read(7, 0), 13);
        assert_eq!(disk.read(7, 1), -25);
    }
}
