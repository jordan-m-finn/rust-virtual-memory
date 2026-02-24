use crate::constants::*;

/// Physical Memory - simulates the main memory hardware
pub struct PhysicalMemory {
    /// The actual memory array - using Box because 524K × 4 bytes = 2MB is too large for stack
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
    /// Stack of free frame numbers (LIFO allocation)
    free_frames: Vec<u32>,
}

impl FreeFrameList {
    /// Create a new free frame list with all frames (2-1023) initially free
    pub fn new() -> Self {
        // Start with frames 2 through NUM_FRAMES-1 as free
        let free_frames: Vec<u32> = (ST_FRAMES as u32..NUM_FRAMES as u32).rev().collect();

        FreeFrameList { free_frames }
    }

    /// Mark a frame as occupied (not available for allocation)
    pub fn mark_occupied(&mut self, frame: u32) {
        if let Some(pos) = self.free_frames.iter().position(|&f| f == frame) {
            self.free_frames.remove(pos);
        }
    }

    /// Allocate a free frame
    pub fn allocate(&mut self) -> Option<u32> {
        self.free_frames.pop()
    }

    /// Check how many frames are currently free
    pub fn free_count(&self) -> usize {
        self.free_frames.len()
    }

    /// Check if a specific frame is free
    pub fn is_free(&self, frame: u32) -> bool {
        self.free_frames.contains(&frame)
    }

    /// Get the next frame that would be allocated (without allocating it)
    pub fn peek_next(&self) -> Option<u32> {
        self.free_frames.last().copied()
    }
}

impl Default for FreeFrameList {
    fn default() -> Self {
        Self::new()
    }
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

    // =========================================================================
    // FreeFrameList tests
    // =========================================================================

    #[test]
    fn test_free_frame_list_initialization() {
        let ffl = FreeFrameList::new();

        // Should have NUM_FRAMES - ST_FRAMES free frames (1024 - 2 = 1022)
        assert_eq!(ffl.free_count(), NUM_FRAMES - ST_FRAMES);

        // Frames 0 and 1 should NOT be free (reserved for ST)
        assert!(!ffl.is_free(0));
        assert!(!ffl.is_free(1));

        // Frame 2 should be free (first available)
        assert!(ffl.is_free(2));

        // Frame 1023 should be free (last frame)
        assert!(ffl.is_free(1023));
    }

    #[test]
    fn test_free_frame_list_allocate() {
        let mut ffl = FreeFrameList::new();
        let initial_count = ffl.free_count();

        // Allocate a frame
        let frame = ffl.allocate();
        assert!(frame.is_some());

        // Count should decrease
        assert_eq!(ffl.free_count(), initial_count - 1);

        // Allocated frame should no longer be free
        let allocated = frame.unwrap();
        assert!(!ffl.is_free(allocated));
    }

    #[test]
    fn test_free_frame_list_allocate_order() {
        let mut ffl = FreeFrameList::new();

        // Should allocate low-numbered frames first
        let f1 = ffl.allocate().unwrap();
        let f2 = ffl.allocate().unwrap();
        let f3 = ffl.allocate().unwrap();

        // First three free frames are 2, 3, 4
        assert_eq!(f1, 2);
        assert_eq!(f2, 3);
        assert_eq!(f3, 4);
    }

    #[test]
    fn test_free_frame_list_mark_occupied() {
        let mut ffl = FreeFrameList::new();

        // Mark frames 3, 10, 13 as occupied (from the spec example)
        ffl.mark_occupied(3);
        ffl.mark_occupied(10);
        ffl.mark_occupied(13);

        // These frames should no longer be free
        assert!(!ffl.is_free(3));
        assert!(!ffl.is_free(10));
        assert!(!ffl.is_free(13));

        // Count should decrease by 3
        assert_eq!(ffl.free_count(), NUM_FRAMES - ST_FRAMES - 3);
    }

    #[test]
    fn test_free_frame_list_allocate_after_mark_occupied() {
        let mut ffl = FreeFrameList::new();

        // From spec example: frames 0, 1, 3, 10, 13 are occupied
        // (0, 1 are ST, so we just mark 3, 10, 13)
        ffl.mark_occupied(3);
        ffl.mark_occupied(10);
        ffl.mark_occupied(13);

        // Next allocation should be frame 2 (first free)
        assert_eq!(ffl.allocate(), Some(2));

        // Then frame 4 (3 is occupied)
        assert_eq!(ffl.allocate(), Some(4));

        // Then frame 5
        assert_eq!(ffl.allocate(), Some(5));
    }

    #[test]
    fn test_free_frame_list_spec_example() {
        // From spec: "frames 0, 1, 3, 10, 13 are occupied"
        // "The following VA translations use the free frames 2, 4, 5"
        let mut ffl = FreeFrameList::new();

        // Mark frames as occupied per spec
        ffl.mark_occupied(3);
        ffl.mark_occupied(10);
        ffl.mark_occupied(13);

        // Allocations should give us 2, 4, 5 in order
        assert_eq!(ffl.allocate(), Some(2));
        assert_eq!(ffl.allocate(), Some(4));
        assert_eq!(ffl.allocate(), Some(5));
    }

    #[test]
    fn test_free_frame_list_peek_next() {
        let mut ffl = FreeFrameList::new();

        // Peek should return 2 (first free frame)
        assert_eq!(ffl.peek_next(), Some(2));

        // Peek doesn't consume
        assert_eq!(ffl.peek_next(), Some(2));

        // After allocation, peek returns next
        ffl.allocate();
        assert_eq!(ffl.peek_next(), Some(3));
    }

    #[test]
    fn test_free_frame_list_mark_occupied_idempotent() {
        let mut ffl = FreeFrameList::new();
        let initial_count = ffl.free_count();

        // Marking same frame twice should only remove it once
        ffl.mark_occupied(5);
        ffl.mark_occupied(5);

        assert_eq!(ffl.free_count(), initial_count - 1);
    }

    #[test]
    fn test_free_frame_list_mark_st_frames() {
        let mut ffl = FreeFrameList::new();
        let initial_count = ffl.free_count();

        // Marking ST frames (0, 1) should have no effect - they were never free
        ffl.mark_occupied(0);
        ffl.mark_occupied(1);

        assert_eq!(ffl.free_count(), initial_count);
    }
}
