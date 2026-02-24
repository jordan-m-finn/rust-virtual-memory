//! Physical Memory and Disk structures
//!
//! This module contains:
//! - PhysicalMemory: The PM[524288] array simulating RAM
//! - Disk: The D[1024][512] array simulating the paging disk
//! - FreeFrameList: Tracking available frames for demand paging (Commit 7)

use crate::constants::*;

/// Physical Memory - simulates the main memory hardware
///
/// PM is organized as 1024 frames of 512 words each (524,288 words total).
/// Frames 0 and 1 are reserved for the Segment Table (ST).
///
/// # Memory Layout
/// ```text
/// Frame 0-1:  Segment Table (1024 words, 512 entries × 2 words each)
/// Frame 2+:   Page Tables and Pages
/// ```
///
/// # Segment Table Entry Format
/// Each ST entry occupies 2 consecutive words:
/// - PM[2*s]:     Size of segment s (number of valid words)
/// - PM[2*s + 1]: Frame number of PT for segment s (or negative disk block)
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
    ///
    /// # Arguments
    /// * `address` - Physical address (0 to PM_SIZE-1)
    ///
    /// # Panics
    /// Panics if address is out of bounds
    #[inline]
    pub fn read(&self, address: usize) -> i32 {
        self.data[address]
    }

    /// Write a word to physical memory
    ///
    /// # Arguments
    /// * `address` - Physical address (0 to PM_SIZE-1)
    /// * `value` - Value to write
    ///
    /// # Panics
    /// Panics if address is out of bounds
    #[inline]
    pub fn write(&mut self, address: usize, value: i32) {
        self.data[address] = value;
    }

    /// Get the size of a segment from the Segment Table
    ///
    /// # Arguments
    /// * `segment` - Segment number (0 to MAX_SEGMENTS-1)
    ///
    /// # Returns
    /// The size field from ST entry (number of valid words in segment)
    #[inline]
    pub fn get_segment_size(&self, segment: u32) -> i32 {
        self.data[2 * segment as usize]
    }

    /// Get the PT frame/block for a segment from the Segment Table
    ///
    /// # Arguments
    /// * `segment` - Segment number (0 to MAX_SEGMENTS-1)
    ///
    /// # Returns
    /// - Positive: Frame number where PT resides
    /// - Negative: Disk block number where PT resides (demand paging)
    /// - Zero: Segment does not exist
    #[inline]
    pub fn get_segment_pt_location(&self, segment: u32) -> i32 {
        self.data[2 * segment as usize + 1]
    }

    /// Set a Segment Table entry
    ///
    /// # Arguments
    /// * `segment` - Segment number
    /// * `size` - Size of the segment in words
    /// * `pt_location` - Frame number (positive) or disk block (negative) of PT
    pub fn set_segment_entry(&mut self, segment: u32, size: i32, pt_location: i32) {
        let base = 2 * segment as usize;
        self.data[base] = size;
        self.data[base + 1] = pt_location;
    }

    /// Get a Page Table entry
    ///
    /// # Arguments
    /// * `pt_frame` - Frame number where the PT resides
    /// * `page` - Page number (index into PT)
    ///
    /// # Returns
    /// - Positive: Frame number where page resides
    /// - Negative: Disk block number where page resides (demand paging)
    /// - Zero: Page does not exist
    #[inline]
    pub fn get_page_frame(&self, pt_frame: i32, page: u32) -> i32 {
        let pt_base = pt_frame as usize * PAGE_SIZE;
        self.data[pt_base + page as usize]
    }

    /// Set a Page Table entry
    ///
    /// # Arguments
    /// * `pt_frame` - Frame number where the PT resides
    /// * `page` - Page number (index into PT)
    /// * `frame_location` - Frame number (positive) or disk block (negative)
    pub fn set_page_entry(&mut self, pt_frame: i32, page: u32, frame_location: i32) {
        let pt_base = pt_frame as usize * PAGE_SIZE;
        self.data[pt_base + page as usize] = frame_location;
    }

    /// Calculate the starting address of a frame
    ///
    /// # Arguments
    /// * `frame` - Frame number (0 to NUM_FRAMES-1)
    ///
    /// # Returns
    /// The physical address where the frame starts
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
///
/// Organized as 1024 blocks of 512 words each.
/// Non-resident pages and page tables are stored here.
///
/// # Usage
/// When a page fault occurs (negative value in ST or PT entry),
/// the absolute value indicates the disk block to load from.
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
    ///
    /// # Arguments
    /// * `block` - Block number (0 to DISK_BLOCKS-1)
    /// * `offset` - Offset within block (0 to BLOCK_SIZE-1)
    #[inline]
    pub fn read(&self, block: usize, offset: usize) -> i32 {
        self.data[block][offset]
    }

    /// Write a word to disk
    ///
    /// # Arguments
    /// * `block` - Block number
    /// * `offset` - Offset within block
    /// * `value` - Value to write
    #[inline]
    pub fn write(&mut self, block: usize, offset: usize, value: i32) {
        self.data[block][offset] = value;
    }

    /// Read an entire block from disk into physical memory
    ///
    /// This simulates the `read_block(b, m)` operation from the spec:
    /// copies entire block D[b] into PM starting at location m.
    ///
    /// # Arguments
    /// * `block` - Disk block number to read
    /// * `pm` - Physical memory to write into
    /// * `pm_start` - Starting address in PM (should be frame-aligned)
    pub fn read_block(&self, block: usize, pm: &mut PhysicalMemory, pm_start: usize) {
        for i in 0..BLOCK_SIZE {
            pm.write(pm_start + i, self.data[block][i]);
        }
    }

    /// Load a page table from disk into a physical memory frame
    ///
    /// This is a convenience method for handling PT page faults.
    ///
    /// # Arguments
    /// * `disk_block` - The disk block containing the PT (absolute value of ST entry)
    /// * `frame` - The frame number to load into
    /// * `pm` - Physical memory
    ///
    /// # Example
    /// ```ignore
    /// // ST entry for segment 9 is -7, meaning PT is in disk block 7
    /// // We allocate frame 5 and load the PT there
    /// disk.load_pt_from_disk(7, 5, &mut pm);
    /// // Now update ST: pm.set_segment_entry(9, size, 5);
    /// ```
    pub fn load_pt_from_disk(&self, disk_block: usize, frame: u32, pm: &mut PhysicalMemory) {
        let pm_start = PhysicalMemory::frame_to_address(frame as i32);
        self.read_block(disk_block, pm, pm_start);
    }

    /// Load a page from disk into a physical memory frame
    ///
    /// This is a convenience method for handling page faults.
    /// Note: In this simulation, pages on disk are represented implicitly
    /// (they contain zeros), so this just ensures the frame is zeroed.
    /// In a real system, this would copy actual page contents.
    ///
    /// # Arguments
    /// * `disk_block` - The disk block containing the page (absolute value of PT entry)
    /// * `frame` - The frame number to load into
    /// * `pm` - Physical memory
    ///
    /// # Example
    /// ```ignore
    /// // PT entry for page 1 of segment 8 is -20, meaning page is in disk block 20
    /// // We allocate frame 4 and load the page there
    /// disk.load_page_from_disk(20, 4, &mut pm);
    /// // Now update PT: pm.set_page_entry(pt_frame, 1, 4);
    /// ```
    pub fn load_page_from_disk(&self, disk_block: usize, frame: u32, pm: &mut PhysicalMemory) {
        let pm_start = PhysicalMemory::frame_to_address(frame as i32);
        self.read_block(disk_block, pm, pm_start);
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
///
/// Used during demand paging to find free frames when
/// loading pages or page tables from disk.
///
/// # Design
/// We use a simple Vec-based stack of free frame numbers.
/// Frames 0 and 1 are never free (reserved for Segment Table).
///
/// # Initialization
/// The list starts with all frames (2-1023) marked as free,
/// then frames are marked as occupied based on the init file.
pub struct FreeFrameList {
    /// Stack of free frame numbers (LIFO allocation)
    free_frames: Vec<u32>,
}

impl FreeFrameList {
    /// Create a new free frame list with all frames (2-1023) initially free
    ///
    /// Frames 0 and 1 are reserved for the Segment Table and are never free.
    pub fn new() -> Self {
        // Start with frames 2 through NUM_FRAMES-1 as free
        // We push in reverse order so that lower-numbered frames are allocated first
        // (matches the expected behavior in the spec examples)
        let free_frames: Vec<u32> = (ST_FRAMES as u32..NUM_FRAMES as u32).rev().collect();

        FreeFrameList { free_frames }
    }

    /// Mark a frame as occupied (not available for allocation)
    ///
    /// Called during initialization when a frame is specified in the init file.
    ///
    /// # Arguments
    /// * `frame` - Frame number to mark as occupied
    ///
    /// # Note
    /// This is O(n) but only called during initialization, not during translation.
    pub fn mark_occupied(&mut self, frame: u32) {
        if let Some(pos) = self.free_frames.iter().position(|&f| f == frame) {
            self.free_frames.remove(pos);
        }
    }

    /// Allocate a free frame
    ///
    /// # Returns
    /// The frame number of an available frame, or None if no frames are free.
    ///
    /// # Note
    /// According to the spec, "PM will always have a sufficient number of
    /// free frames available so that no page replacement algorithm is needed."
    /// So in practice, this should never return None.
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
    ///
    /// Useful for testing and debugging.
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
    fn test_load_pt_from_disk() {
        let mut disk = Disk::new();
        let mut pm = PhysicalMemory::new();

        // Set up disk block 7 with PT data for segment 9
        // Page 0 -> frame 13, Page 1 -> disk block 25
        disk.write(7, 0, 13);
        disk.write(7, 1, -25);

        // Load PT from disk block 7 into frame 5
        disk.load_pt_from_disk(7, 5, &mut pm);

        // Verify PT is now in PM at frame 5
        assert_eq!(pm.get_page_frame(5, 0), 13);
        assert_eq!(pm.get_page_frame(5, 1), -25);
    }

    #[test]
    fn test_load_page_from_disk() {
        let mut disk = Disk::new();
        let mut pm = PhysicalMemory::new();

        // Set up disk block 20 with some page data
        disk.write(20, 0, 100);
        disk.write(20, 100, 200);
        disk.write(20, 511, 999);

        // Load page from disk block 20 into frame 4
        disk.load_page_from_disk(20, 4, &mut pm);

        // Verify page data is now in PM at frame 4 (address 4*512 = 2048)
        assert_eq!(pm.read(2048), 100);
        assert_eq!(pm.read(2148), 200);
        assert_eq!(pm.read(2048 + 511), 999);
    }

    #[test]
    fn test_demand_paging_pt_fault_simulation() {
        // Simulate handling a PT page fault:
        // Segment 9 has PT on disk block 7, we need to load it to frame 5

        let mut disk = Disk::new();
        let mut pm = PhysicalMemory::new();
        let mut ffl = FreeFrameList::new();

        // Initial state: ST says segment 9's PT is on disk block 7
        pm.set_segment_entry(9, 5000, -7);

        // PT on disk block 7 says: page 0 -> frame 13, page 1 -> disk block 25
        disk.write(7, 0, 13);
        disk.write(7, 1, -25);

        // Mark frame 13 as occupied (page 0 is resident)
        ffl.mark_occupied(13);

        // --- Page fault handling ---

        // 1. PT is not resident (PT location is -7)
        let pt_location = pm.get_segment_pt_location(9);
        assert!(pt_location < 0, "PT should be on disk");

        // 2. Get disk block number
        let disk_block = (-pt_location) as usize;
        assert_eq!(disk_block, 7);

        // 3. Allocate a free frame for the PT
        let pt_frame = ffl.allocate().unwrap();
        assert_eq!(pt_frame, 2); // First free frame

        // 4. Load PT from disk into the allocated frame
        disk.load_pt_from_disk(disk_block, pt_frame, &mut pm);

        // 5. Update ST to point to the new frame
        pm.set_segment_entry(9, 5000, pt_frame as i32);

        // --- Verification ---

        // ST should now point to frame 2
        assert_eq!(pm.get_segment_pt_location(9), 2);

        // PT entries should be accessible
        assert_eq!(pm.get_page_frame(2, 0), 13);   // Page 0 in frame 13
        assert_eq!(pm.get_page_frame(2, 1), -25);  // Page 1 on disk block 25
    }

    #[test]
    fn test_demand_paging_page_fault_simulation() {
        // Simulate handling a page fault:
        // Page 1 of segment 8 is on disk block 20, we need to load it to frame 4

        let disk = Disk::new();
        let mut pm = PhysicalMemory::new();
        let mut ffl = FreeFrameList::new();

        // Initial state: segment 8 has PT in frame 3
        pm.set_segment_entry(8, 4000, 3);

        // PT says: page 0 -> frame 10, page 1 -> disk block 20
        pm.set_page_entry(3, 0, 10);
        pm.set_page_entry(3, 1, -20);

        // Mark occupied frames
        ffl.mark_occupied(3);
        ffl.mark_occupied(10);

        // --- Page fault handling ---

        // 1. Page is not resident (PT entry is -20)
        let pt_frame = pm.get_segment_pt_location(8);
        let page_location = pm.get_page_frame(pt_frame, 1);
        assert!(page_location < 0, "Page should be on disk");

        // 2. Get disk block number
        let disk_block = (-page_location) as usize;
        assert_eq!(disk_block, 20);

        // 3. Allocate a free frame for the page
        let new_frame = ffl.allocate().unwrap();
        assert_eq!(new_frame, 2); // First free frame

        // 4. Load page from disk into the allocated frame
        disk.load_page_from_disk(disk_block, new_frame, &mut pm);

        // 5. Update PT to point to the new frame
        pm.set_page_entry(pt_frame, 1, new_frame as i32);

        // --- Verification ---

        // PT entry should now point to frame 2
        assert_eq!(pm.get_page_frame(3, 1), 2);
    }

    #[test]
    fn test_demand_paging_full_scenario() {
        // Complete simulation matching the spec example:
        // Init: 8 4000 3 9 5000 -7
        //       8 0 10 8 1 -20 9 0 13 9 1 -25
        // VAs: 2097162, 2097674, 2359306, 2359818
        // Expected PAs: 5130, 1034, 6666, 2570

        let mut disk = Disk::new();
        let mut pm = PhysicalMemory::new();
        let mut ffl = FreeFrameList::new();

        // --- Initialization ---

        // Segment 8: size=4000, PT in frame 3
        pm.set_segment_entry(8, 4000, 3);
        ffl.mark_occupied(3);

        // Segment 9: size=5000, PT in disk block 7
        pm.set_segment_entry(9, 5000, -7);

        // PT for segment 8 (in frame 3)
        pm.set_page_entry(3, 0, 10);   // Page 0 -> frame 10
        pm.set_page_entry(3, 1, -20);  // Page 1 -> disk block 20
        ffl.mark_occupied(10);

        // PT for segment 9 (on disk block 7)
        disk.write(7, 0, 13);   // Page 0 -> frame 13
        disk.write(7, 1, -25);  // Page 1 -> disk block 25
        ffl.mark_occupied(13);

        // Verify initial free frames: should be 2, 4, 5, 6, ... (3, 10, 13 occupied)
        assert!(!ffl.is_free(3));
        assert!(!ffl.is_free(10));
        assert!(!ffl.is_free(13));
        assert!(ffl.is_free(2));
        assert!(ffl.is_free(4));
        assert!(ffl.is_free(5));

        // --- VA 1: 2097162 = (8, 0, 10) ---
        // PT resident, page resident -> no faults
        // PA = frame_10 * 512 + 10 = 5120 + 10 = 5130
        let pt_loc_8 = pm.get_segment_pt_location(8);
        assert_eq!(pt_loc_8, 3); // PT is resident
        let page_frame = pm.get_page_frame(pt_loc_8, 0);
        assert_eq!(page_frame, 10); // Page is resident
        let pa1 = page_frame * PAGE_SIZE as i32 + 10;
        assert_eq!(pa1, 5130);

        // --- VA 2: 2097674 = (8, 1, 10) ---
        // PT resident, page NOT resident (disk block 20)
        // Need to allocate frame 2, load page, PA = 2 * 512 + 10 = 1034
        let page_loc = pm.get_page_frame(pt_loc_8, 1);
        assert_eq!(page_loc, -20); // On disk

        let new_frame = ffl.allocate().unwrap();
        assert_eq!(new_frame, 2);

        disk.load_page_from_disk(20, new_frame, &mut pm);
        pm.set_page_entry(pt_loc_8, 1, new_frame as i32);

        let pa2 = new_frame as i32 * PAGE_SIZE as i32 + 10;
        assert_eq!(pa2, 1034);

        // --- VA 3: 2359306 = (9, 0, 10) ---
        // PT NOT resident (disk block 7), page resident (frame 13)
        // Need to allocate frame 4 for PT, load PT
        // PA = frame_13 * 512 + 10 = 6656 + 10 = 6666
        let pt_loc_9 = pm.get_segment_pt_location(9);
        assert_eq!(pt_loc_9, -7); // PT on disk

        let pt_frame = ffl.allocate().unwrap();
        assert_eq!(pt_frame, 4);

        disk.load_pt_from_disk(7, pt_frame, &mut pm);
        pm.set_segment_entry(9, 5000, pt_frame as i32);

        // Now PT is resident, check page 0
        let page_frame_9_0 = pm.get_page_frame(pt_frame as i32, 0);
        assert_eq!(page_frame_9_0, 13); // Page is resident

        let pa3 = page_frame_9_0 * PAGE_SIZE as i32 + 10;
        assert_eq!(pa3, 6666);

        // --- VA 4: 2359818 = (9, 1, 10) ---
        // PT now resident (frame 4), page NOT resident (disk block 25)
        // Need to allocate frame 5 for page
        // PA = 5 * 512 + 10 = 2560 + 10 = 2570
        let page_loc_9_1 = pm.get_page_frame(pt_frame as i32, 1);
        assert_eq!(page_loc_9_1, -25); // On disk

        let page_frame_new = ffl.allocate().unwrap();
        assert_eq!(page_frame_new, 5);

        disk.load_page_from_disk(25, page_frame_new, &mut pm);
        pm.set_page_entry(pt_frame as i32, 1, page_frame_new as i32);

        let pa4 = page_frame_new as i32 * PAGE_SIZE as i32 + 10;
        assert_eq!(pa4, 2570);

        // --- Final verification ---
        assert_eq!(pa1, 5130);
        assert_eq!(pa2, 1034);
        assert_eq!(pa3, 6666);
        assert_eq!(pa4, 2570);
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
