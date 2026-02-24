//! Virtual Address Translation
//!
//! This module handles:
//! - Decomposing a VA into its components (s, p, w, pw)
//! - Translating a VA to a PA using the segment and page tables
//! - Handling page faults during demand paging

use crate::constants::*;

/// Represents the decomposed components of a Virtual Address
///
/// A 32-bit VA is split into:
/// - s (9 bits): segment number - index into Segment Table
/// - p (9 bits): page number - index into Page Table  
/// - w (9 bits): offset within the page
/// - pw (18 bits): offset within the segment (p concatenated with w)
///
/// ```text
/// 32-bit Virtual Address (only 27 bits used):
/// ┌─────────┬─────────┬─────────┬─────────┐
/// │ unused  │    s    │    p    │    w    │
/// │ (5 bits)│ (9 bits)│ (9 bits)│ (9 bits)│
/// └─────────┴─────────┴─────────┴─────────┘
///   bits 27-31  18-26     9-17      0-8
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualAddress {
    /// The original 32-bit virtual address
    pub va: u32,
    /// Segment number (bits 18-26)
    pub s: u32,
    /// Page number (bits 9-17)
    pub p: u32,
    /// Offset within page (bits 0-8)
    pub w: u32,
    /// Offset within segment (bits 0-17), used for bounds checking
    pub pw: u32,
}

impl VirtualAddress {
    /// Decompose a raw VA into its components
    ///
    /// # Arguments
    /// * `va` - The 32-bit virtual address to decompose
    ///
    /// # Returns
    /// A VirtualAddress struct with all components extracted
    ///
    /// # Bit Manipulation
    /// - `s`:  `va >> 18` — shift right to discard p and w
    /// - `p`:  `(va >> 9) & 0x1FF` — shift right to discard w, mask to keep 9 bits
    /// - `w`:  `va & 0x1FF` — mask to keep lower 9 bits
    /// - `pw`: `va & 0x3FFFF` — mask to keep lower 18 bits
    ///
    /// # Example
    /// ```
    /// use vm_manager::VirtualAddress;
    /// 
    /// let va = VirtualAddress::from_raw(789002);
    /// assert_eq!(va.s, 3);
    /// assert_eq!(va.p, 5);
    /// assert_eq!(va.w, 10);
    /// ```
    pub fn from_raw(va: u32) -> Self {
        // Extract segment number: shift right by 18 to discard p and w
        let s = va >> S_SHIFT;

        // Extract page number: shift right by 9 to discard w, then mask
        let p = (va >> P_SHIFT) & P_MASK;

        // Extract offset within page: just mask the lower 9 bits
        let w = va & W_MASK;

        // Extract offset within segment: mask the lower 18 bits (p + w combined)
        let pw = va & PW_MASK;

        VirtualAddress { va, s, p, w, pw }
    }

    /// Compute the sp value (segment + page combined) for TLB lookups
    ///
    /// This combines s and p into a single value used as TLB key.
    /// Not needed for basic translation, but useful for TLB implementation.
    #[inline]
    pub fn sp(&self) -> u32 {
        (self.s << P_BITS) | self.p
    }
}

impl std::fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VA({}) = (s={}, p={}, w={}, pw={})",
            self.va, self.s, self.p, self.w, self.pw
        )
    }
}

/// Result of an address translation attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationResult {
    /// Successfully translated to a physical address
    Success(i32),
    /// VA exceeded segment boundary (pw >= segment size)
    SegmentBoundaryViolation,
    /// Segment does not exist (size and frame are both 0)
    InvalidSegment,
    /// Page does not exist (frame number is 0)
    InvalidPage,
}

impl TranslationResult {
    /// Convert to the output format (-1 for errors, PA otherwise)
    pub fn to_output(&self) -> i32 {
        match self {
            TranslationResult::Success(pa) => *pa,
            _ => INVALID_ADDRESS,
        }
    }
}

/// Translate a virtual address to a physical address (without demand paging)
///
/// # Algorithm
/// 1. Check if segment exists (size > 0)
/// 2. Check bounds: pw must be < segment size
/// 3. Look up PT frame from ST: `pt_frame = PM[2s + 1]`
/// 4. Look up page frame from PT: `page_frame = PM[pt_frame * 512 + p]`
/// 5. Calculate PA: `PA = page_frame * 512 + w`
///
/// # Arguments
/// * `va` - The decomposed virtual address
/// * `pm` - Reference to physical memory
///
/// # Returns
/// The translation result (PA or error)
///
/// # Note
/// This version does NOT handle demand paging. All PT and page entries
/// must be positive (resident in memory). See `translate_with_demand_paging`
/// for the extended version.
pub fn translate(va: &VirtualAddress, pm: &crate::memory::PhysicalMemory) -> TranslationResult {
    // Step 1: Get segment table entry
    let segment_size = pm.get_segment_size(va.s);
    let pt_location = pm.get_segment_pt_location(va.s);

    // Step 2: Check if segment exists
    // If both size and pt_location are 0, segment doesn't exist
    if segment_size == 0 && pt_location == 0 {
        return TranslationResult::InvalidSegment;
    }

    // Step 3: Check segment boundary
    // pw is the offset within the segment, must be less than segment size
    if va.pw >= segment_size as u32 {
        return TranslationResult::SegmentBoundaryViolation;
    }

    // Step 4: Look up page table entry
    // For basic translation, pt_location must be positive (resident)
    if pt_location <= 0 {
        // In basic mode, negative means not resident - treat as invalid
        // (Demand paging would handle this differently)
        return TranslationResult::InvalidSegment;
    }

    let page_frame = pm.get_page_frame(pt_location, va.p);

    // Step 5: Check if page exists
    if page_frame <= 0 {
        // Page frame of 0 means page doesn't exist
        // Negative would mean on disk (demand paging)
        return TranslationResult::InvalidPage;
    }

    // Step 6: Calculate physical address
    // PA = page_frame * PAGE_SIZE + w
    let pa = page_frame * PAGE_SIZE as i32 + va.w as i32;

    TranslationResult::Success(pa)
}

/// Translate a batch of virtual addresses
///
/// Convenience function that translates multiple VAs and returns
/// the results in output format (PA or -1 for errors).
///
/// # Arguments
/// * `vas` - Slice of raw virtual addresses
/// * `pm` - Reference to physical memory
///
/// # Returns
/// Vector of results (PA or -1 for each VA)
pub fn translate_batch(vas: &[u32], pm: &crate::memory::PhysicalMemory) -> Vec<i32> {
    vas.iter()
        .map(|&va| {
            let va = VirtualAddress::from_raw(va);
            translate(&va, pm).to_output()
        })
        .collect()
}

/// Translate a virtual address to a physical address WITH demand paging
///
/// This version handles page faults by:
/// 1. Detecting non-resident PTs (negative ST entry)
/// 2. Detecting non-resident pages (negative PT entry)
/// 3. Allocating free frames and loading from disk
/// 4. Updating ST/PT entries to reflect new locations
///
/// # Algorithm
/// ```text
/// 1. Check segment boundary (pw < segment_size)
/// 2. If PT not resident (PM[2s+1] < 0):
///    - Allocate frame f1
///    - Load PT from disk block |PM[2s+1]| into frame f1
///    - Update PM[2s+1] = f1
/// 3. If page not resident (PT entry < 0):
///    - Allocate frame f2  
///    - Load page from disk block |PT entry| into frame f2
///    - Update PT entry = f2
/// 4. Return PA = page_frame * 512 + w
/// ```
///
/// # Arguments
/// * `va` - The decomposed virtual address
/// * `pm` - Mutable reference to physical memory
/// * `disk` - Reference to the paging disk
/// * `ffl` - Mutable reference to the free frame list
///
/// # Returns
/// The translation result (PA or error)
pub fn translate_with_demand_paging(
    va: &VirtualAddress,
    pm: &mut crate::memory::PhysicalMemory,
    disk: &crate::memory::Disk,
    ffl: &mut crate::memory::FreeFrameList,
) -> TranslationResult {
    // Step 1: Get segment table entry
    let segment_size = pm.get_segment_size(va.s);
    let mut pt_location = pm.get_segment_pt_location(va.s);

    // Step 2: Check if segment exists
    if segment_size == 0 && pt_location == 0 {
        return TranslationResult::InvalidSegment;
    }

    // Step 3: Check segment boundary
    if va.pw >= segment_size as u32 {
        return TranslationResult::SegmentBoundaryViolation;
    }

    // Step 4: Handle PT page fault if PT is not resident
    if pt_location < 0 {
        // PT is on disk - need to load it
        let disk_block = (-pt_location) as usize;

        // Allocate a free frame for the PT
        let new_frame = match ffl.allocate() {
            Some(f) => f,
            None => return TranslationResult::InvalidSegment, // No free frames (shouldn't happen per spec)
        };

        // Load PT from disk into the new frame
        disk.load_pt_from_disk(disk_block, new_frame, pm);

        // Update ST to point to the new frame
        pm.set_segment_entry(va.s, segment_size, new_frame as i32);

        // Update our local variable
        pt_location = new_frame as i32;
    }

    // Step 5: Look up page table entry
    let mut page_frame = pm.get_page_frame(pt_location, va.p);

    // Step 6: Handle page fault if page is not resident
    if page_frame < 0 {
        // Page is on disk - need to load it
        let disk_block = (-page_frame) as usize;

        // Allocate a free frame for the page
        let new_frame = match ffl.allocate() {
            Some(f) => f,
            None => return TranslationResult::InvalidPage, // No free frames (shouldn't happen per spec)
        };

        // Load page from disk into the new frame
        disk.load_page_from_disk(disk_block, new_frame, pm);

        // Update PT to point to the new frame
        pm.set_page_entry(pt_location, va.p, new_frame as i32);

        // Update our local variable
        page_frame = new_frame as i32;
    }

    // Step 7: Check if page exists (frame number is 0 means no page)
    if page_frame == 0 {
        return TranslationResult::InvalidPage;
    }

    // Step 8: Calculate physical address
    let pa = page_frame * PAGE_SIZE as i32 + va.w as i32;

    TranslationResult::Success(pa)
}

/// Translate a batch of virtual addresses WITH demand paging
///
/// # Arguments
/// * `vas` - Slice of raw virtual addresses
/// * `pm` - Mutable reference to physical memory
/// * `disk` - Reference to the paging disk
/// * `ffl` - Mutable reference to the free frame list
///
/// # Returns
/// Vector of results (PA or -1 for each VA)
pub fn translate_batch_with_demand_paging(
    vas: &[u32],
    pm: &mut crate::memory::PhysicalMemory,
    disk: &crate::memory::Disk,
    ffl: &mut crate::memory::FreeFrameList,
) -> Vec<i32> {
    vas.iter()
        .map(|&va| {
            let va = VirtualAddress::from_raw(va);
            translate_with_demand_paging(&va, pm, disk, ffl).to_output()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_va_decomposition_example_from_spec() {
        // From the spec: VA = 789002 = 000000011 000000101 000001010
        // s = 3, p = 5, w = 10
        let va = VirtualAddress::from_raw(789002);

        assert_eq!(va.s, 3);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 10);
        assert_eq!(va.pw, 5 * 512 + 10); // p * PAGE_SIZE + w = 2570
    }

    #[test]
    fn test_va_decomposition_test_case_1() {
        // From spec test case: VA = 1575424
        // Expected: s=6, since output PA=4608 and frame 9 gives 9*512=4608
        // So w=0, and we need pw < 3000
        let va = VirtualAddress::from_raw(1575424);

        // Let's verify by reconstructing: s=6, p=5, w=0
        // VA = (6 << 18) | (5 << 9) | 0 = 1572864 + 2560 + 0 = 1575424 ✓
        assert_eq!(va.s, 6);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 0);
        assert_eq!(va.pw, 2560); // 5 * 512 + 0
    }

    #[test]
    fn test_va_decomposition_test_case_2() {
        // From spec test case: VA = 1575863
        // Expected PA = 5047 = 9*512 + 439
        // So s=6, p=5, w=439
        let va = VirtualAddress::from_raw(1575863);

        assert_eq!(va.s, 6);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 439);
        assert_eq!(va.pw, 2999); // 5 * 512 + 439 = 2999 (just under 3000)
    }

    #[test]
    fn test_va_decomposition_test_case_3() {
        // From spec test case: VA = 1575864
        // This should fail because pw = 3000 >= segment size of 3000
        let va = VirtualAddress::from_raw(1575864);

        assert_eq!(va.s, 6);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 440);
        assert_eq!(va.pw, 3000); // Exactly at boundary - should fail
    }

    #[test]
    fn test_va_decomposition_demand_paging_case_1() {
        // From demand paging test: VA = 2097162
        // Expected: s=8, p=0, w=10
        let va = VirtualAddress::from_raw(2097162);

        // Verify: (8 << 18) | (0 << 9) | 10 = 2097152 + 0 + 10 = 2097162 ✓
        assert_eq!(va.s, 8);
        assert_eq!(va.p, 0);
        assert_eq!(va.w, 10);
    }

    #[test]
    fn test_va_decomposition_demand_paging_case_2() {
        // From demand paging test: VA = 2097674
        // Expected: s=8, p=1, w=10
        let va = VirtualAddress::from_raw(2097674);

        // Verify: (8 << 18) | (1 << 9) | 10 = 2097152 + 512 + 10 = 2097674 ✓
        assert_eq!(va.s, 8);
        assert_eq!(va.p, 1);
        assert_eq!(va.w, 10);
    }

    #[test]
    fn test_va_decomposition_demand_paging_case_3() {
        // From demand paging test: VA = 2359306
        // Expected: s=9, p=0, w=10
        let va = VirtualAddress::from_raw(2359306);

        // Verify: (9 << 18) | (0 << 9) | 10 = 2359296 + 0 + 10 = 2359306 ✓
        assert_eq!(va.s, 9);
        assert_eq!(va.p, 0);
        assert_eq!(va.w, 10);
    }

    #[test]
    fn test_va_decomposition_demand_paging_case_4() {
        // From demand paging test: VA = 2359818
        // Expected: s=9, p=1, w=10
        let va = VirtualAddress::from_raw(2359818);

        // Verify: (9 << 18) | (1 << 9) | 10 = 2359296 + 512 + 10 = 2359818 ✓
        assert_eq!(va.s, 9);
        assert_eq!(va.p, 1);
        assert_eq!(va.w, 10);
    }

    #[test]
    fn test_va_decomposition_edge_cases() {
        // All zeros
        let va = VirtualAddress::from_raw(0);
        assert_eq!(va.s, 0);
        assert_eq!(va.p, 0);
        assert_eq!(va.w, 0);
        assert_eq!(va.pw, 0);

        // Maximum values for each component (9 bits each = 511 max)
        // VA = (511 << 18) | (511 << 9) | 511
        let max_va = (511 << 18) | (511 << 9) | 511;
        let va = VirtualAddress::from_raw(max_va);
        assert_eq!(va.s, 511);
        assert_eq!(va.p, 511);
        assert_eq!(va.w, 511);
        assert_eq!(va.pw, (511 << 9) | 511); // 262143
    }

    #[test]
    fn test_va_reconstruction() {
        // Verify that decomposition is reversible
        for &original in &[0, 789002, 1575424, 1575863, 2097162, 2359818] {
            let va = VirtualAddress::from_raw(original);
            let reconstructed = (va.s << 18) | (va.p << 9) | va.w;
            assert_eq!(reconstructed, original, "Failed for VA={}", original);
        }
    }

    #[test]
    fn test_pw_calculation() {
        // pw should equal p * PAGE_SIZE + w
        let va = VirtualAddress::from_raw(789002);
        assert_eq!(va.pw, va.p * PAGE_SIZE as u32 + va.w);

        let va = VirtualAddress::from_raw(1575863);
        assert_eq!(va.pw, va.p * PAGE_SIZE as u32 + va.w);
    }

    #[test]
    fn test_sp_calculation() {
        // sp combines segment and page for TLB lookup
        let va = VirtualAddress::from_raw(789002); // s=3, p=5
        assert_eq!(va.sp(), (3 << 9) | 5);

        let va = VirtualAddress::from_raw(2097162); // s=8, p=0
        assert_eq!(va.sp(), (8 << 9) | 0);
    }

    #[test]
    fn test_display() {
        let va = VirtualAddress::from_raw(789002);
        let display = format!("{}", va);
        assert!(display.contains("789002"));
        assert!(display.contains("s=3"));
        assert!(display.contains("p=5"));
        assert!(display.contains("w=10"));
    }

    #[test]
    fn test_translation_result_to_output() {
        assert_eq!(TranslationResult::Success(4608).to_output(), 4608);
        assert_eq!(TranslationResult::SegmentBoundaryViolation.to_output(), -1);
        assert_eq!(TranslationResult::InvalidSegment.to_output(), -1);
        assert_eq!(TranslationResult::InvalidPage.to_output(), -1);
    }

    // =========================================================================
    // Translation tests - Simple test case from spec
    // =========================================================================

    fn setup_simple_test_memory() -> crate::memory::PhysicalMemory {
        // From spec: init file contains:
        // Line 1: 6 3000 4
        // Line 2: 6 5 9
        let mut pm = crate::memory::PhysicalMemory::new();

        // Segment 6: size=3000, PT in frame 4
        pm.set_segment_entry(6, 3000, 4);

        // Page 5 of segment 6 is in frame 9
        pm.set_page_entry(4, 5, 9);

        pm
    }

    #[test]
    fn test_translate_simple_case_1() {
        // VA = 1575424, expected PA = 4608
        let pm = setup_simple_test_memory();
        let va = VirtualAddress::from_raw(1575424);

        // Verify decomposition: s=6, p=5, w=0, pw=2560
        assert_eq!(va.s, 6);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 0);
        assert_eq!(va.pw, 2560);

        let result = translate(&va, &pm);

        // PA = frame_9 * 512 + 0 = 4608
        assert_eq!(result, TranslationResult::Success(4608));
        assert_eq!(result.to_output(), 4608);
    }

    #[test]
    fn test_translate_simple_case_2() {
        // VA = 1575863, expected PA = 5047
        let pm = setup_simple_test_memory();
        let va = VirtualAddress::from_raw(1575863);

        // Verify decomposition: s=6, p=5, w=439, pw=2999
        assert_eq!(va.s, 6);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 439);
        assert_eq!(va.pw, 2999); // Just under 3000

        let result = translate(&va, &pm);

        // PA = frame_9 * 512 + 439 = 4608 + 439 = 5047
        assert_eq!(result, TranslationResult::Success(5047));
        assert_eq!(result.to_output(), 5047);
    }

    #[test]
    fn test_translate_simple_case_3_boundary_violation() {
        // VA = 1575864, expected output = -1 (error)
        let pm = setup_simple_test_memory();
        let va = VirtualAddress::from_raw(1575864);

        // Verify decomposition: s=6, p=5, w=440, pw=3000
        assert_eq!(va.s, 6);
        assert_eq!(va.p, 5);
        assert_eq!(va.w, 440);
        assert_eq!(va.pw, 3000); // Exactly at boundary - should fail

        let result = translate(&va, &pm);

        // pw (3000) >= segment_size (3000), so error
        assert_eq!(result, TranslationResult::SegmentBoundaryViolation);
        assert_eq!(result.to_output(), -1);
    }

    #[test]
    fn test_translate_full_simple_test_case() {
        // Complete test case from spec:
        // Input: 1575424 1575863 1575864
        // Output: 4608 5047 -1
        let pm = setup_simple_test_memory();

        let vas = vec![1575424, 1575863, 1575864];
        let results = translate_batch(&vas, &pm);

        assert_eq!(results, vec![4608, 5047, -1]);
    }

    // =========================================================================
    // Additional translation tests
    // =========================================================================

    #[test]
    fn test_translate_invalid_segment() {
        // Try to access segment that doesn't exist
        let pm = setup_simple_test_memory();

        // Segment 7 doesn't exist (not initialized)
        let va = VirtualAddress::from_raw((7 << 18) | (0 << 9) | 0);
        assert_eq!(va.s, 7);

        let result = translate(&va, &pm);
        assert_eq!(result, TranslationResult::InvalidSegment);
    }

    #[test]
    fn test_translate_invalid_page() {
        // Access page that doesn't exist within valid segment
        let pm = setup_simple_test_memory();

        // Page 0 of segment 6 doesn't exist (only page 5 was set up)
        let va = VirtualAddress::from_raw((6 << 18) | (0 << 9) | 0);
        assert_eq!(va.s, 6);
        assert_eq!(va.p, 0);

        let result = translate(&va, &pm);
        assert_eq!(result, TranslationResult::InvalidPage);
    }

    #[test]
    fn test_translate_multiple_segments() {
        let mut pm = crate::memory::PhysicalMemory::new();

        // Set up two segments
        pm.set_segment_entry(3, 1024, 5);  // Segment 3: size=1024, PT in frame 5
        pm.set_segment_entry(7, 2048, 6);  // Segment 7: size=2048, PT in frame 6

        // Set up pages
        pm.set_page_entry(5, 0, 10);  // Seg 3, page 0 -> frame 10
        pm.set_page_entry(5, 1, 11);  // Seg 3, page 1 -> frame 11
        pm.set_page_entry(6, 0, 20);  // Seg 7, page 0 -> frame 20

        // Test segment 3, page 0, offset 100
        let va1 = VirtualAddress::from_raw((3 << 18) | (0 << 9) | 100);
        let result1 = translate(&va1, &pm);
        assert_eq!(result1, TranslationResult::Success(10 * 512 + 100));

        // Test segment 3, page 1, offset 50
        let va2 = VirtualAddress::from_raw((3 << 18) | (1 << 9) | 50);
        let result2 = translate(&va2, &pm);
        assert_eq!(result2, TranslationResult::Success(11 * 512 + 50));

        // Test segment 7, page 0, offset 0
        let va3 = VirtualAddress::from_raw((7 << 18) | (0 << 9) | 0);
        let result3 = translate(&va3, &pm);
        assert_eq!(result3, TranslationResult::Success(20 * 512));
    }

    #[test]
    fn test_translate_boundary_edge_cases() {
        let mut pm = crate::memory::PhysicalMemory::new();

        // Segment with size exactly 512 (one page)
        pm.set_segment_entry(1, 512, 3);
        pm.set_page_entry(3, 0, 8);

        // pw = 511 should work (just under 512)
        let va1 = VirtualAddress::from_raw((1 << 18) | (0 << 9) | 511);
        assert_eq!(va1.pw, 511);
        let result1 = translate(&va1, &pm);
        assert_eq!(result1, TranslationResult::Success(8 * 512 + 511));

        // pw = 512 should fail (at boundary)
        let va2 = VirtualAddress::from_raw((1 << 18) | (1 << 9) | 0);
        assert_eq!(va2.pw, 512);
        let result2 = translate(&va2, &pm);
        assert_eq!(result2, TranslationResult::SegmentBoundaryViolation);
    }

    #[test]
    fn test_translate_batch_empty() {
        let pm = setup_simple_test_memory();
        let results = translate_batch(&[], &pm);
        assert!(results.is_empty());
    }

    #[test]
    fn test_translate_with_init_data() {
        // Integration test using InitData to set up memory
        use crate::io::InitData;
        use crate::memory::{Disk, PhysicalMemory};

        let init_content = "6 3000 4\n6 5 9";
        let init = InitData::parse(init_content).unwrap();

        let mut pm = PhysicalMemory::new();
        let mut disk = Disk::new();
        let _ffl = init.apply(&mut pm, &mut disk);

        // Now translate
        let vas = vec![1575424, 1575863, 1575864];
        let results = translate_batch(&vas, &pm);

        assert_eq!(results, vec![4608, 5047, -1]);
    }

    // =========================================================================
    // Demand paging translation tests
    // =========================================================================

    fn setup_demand_paging_memory() -> (
        crate::memory::PhysicalMemory,
        crate::memory::Disk,
        crate::memory::FreeFrameList,
    ) {
        // From spec demand paging test case:
        // Init: 8 4000 3 9 5000 -7
        //       8 0 10 8 1 -20 9 0 13 9 1 -25
        use crate::io::InitData;

        let init_content = "8 4000 3 9 5000 -7\n8 0 10 8 1 -20 9 0 13 9 1 -25";
        let init = InitData::parse(init_content).unwrap();

        let mut pm = crate::memory::PhysicalMemory::new();
        let mut disk = crate::memory::Disk::new();
        let ffl = init.apply(&mut pm, &mut disk);

        (pm, disk, ffl)
    }

    #[test]
    fn test_demand_paging_va1_no_faults() {
        // VA = 2097162 = (8, 0, 10)
        // PT resident (frame 3), page resident (frame 10)
        // Expected PA = 10 * 512 + 10 = 5130
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        let va = VirtualAddress::from_raw(2097162);
        assert_eq!(va.s, 8);
        assert_eq!(va.p, 0);
        assert_eq!(va.w, 10);

        let result = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        assert_eq!(result, TranslationResult::Success(5130));
    }

    #[test]
    fn test_demand_paging_va2_page_fault() {
        // VA = 2097674 = (8, 1, 10)
        // PT resident (frame 3), page NOT resident (disk block 20)
        // Should allocate frame 2 for page
        // Expected PA = 2 * 512 + 10 = 1034
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        let va = VirtualAddress::from_raw(2097674);
        assert_eq!(va.s, 8);
        assert_eq!(va.p, 1);
        assert_eq!(va.w, 10);

        // Before translation, PT entry should be negative
        assert_eq!(pm.get_page_frame(3, 1), -20);

        let result = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        assert_eq!(result, TranslationResult::Success(1034));

        // After translation, PT entry should be updated to frame 2
        assert_eq!(pm.get_page_frame(3, 1), 2);
    }

    #[test]
    fn test_demand_paging_va3_pt_fault() {
        // VA = 2359306 = (9, 0, 10)
        // PT NOT resident (disk block 7), page resident (frame 13)
        // Should allocate frame 2 for PT
        // Expected PA = 13 * 512 + 10 = 6666
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        let va = VirtualAddress::from_raw(2359306);
        assert_eq!(va.s, 9);
        assert_eq!(va.p, 0);
        assert_eq!(va.w, 10);

        // Before translation, ST entry should be negative
        assert_eq!(pm.get_segment_pt_location(9), -7);

        let result = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        assert_eq!(result, TranslationResult::Success(6666));

        // After translation, ST entry should be updated to a positive frame number
        let pt_location = pm.get_segment_pt_location(9);
        assert!(pt_location > 0);
        assert_eq!(pt_location, 2); // First free frame was 2
    }

    #[test]
    fn test_demand_paging_va4_pt_and_page_fault() {
        // VA = 2359818 = (9, 1, 10)
        // PT NOT resident (disk block 7), page NOT resident (disk block 25)
        // Should allocate frame 2 for PT, frame 4 for page
        // Expected PA = 4 * 512 + 10 = 2058
        // 
        // BUT WAIT - the spec says expected output is 2570 = 5 * 512 + 10
        // This is because VA3 would have been processed first, allocating frame 2 for PT
        // Then VA4 sees PT already loaded, only needs to allocate frame for page
        //
        // When processed in isolation (PT not yet loaded), we get:
        // - Frame 2 for PT
        // - Frame 4 for page  
        // - PA = 4 * 512 + 10 = 2058
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        let va = VirtualAddress::from_raw(2359818);
        assert_eq!(va.s, 9);
        assert_eq!(va.p, 1);
        assert_eq!(va.w, 10);

        let result = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        // Frame 2 for PT, frame 4 for page
        assert_eq!(result, TranslationResult::Success(2058));
    }

    #[test]
    fn test_demand_paging_full_sequence() {
        // Test all 4 VAs in sequence, exactly matching spec
        // Input: 2097162 2097674 2359306 2359818
        // Expected output: 5130 1034 6666 2570
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        let vas = vec![2097162, 2097674, 2359306, 2359818];
        let results = translate_batch_with_demand_paging(&vas, &mut pm, &disk, &mut ffl);

        assert_eq!(results, vec![5130, 1034, 6666, 2570]);
    }

    #[test]
    fn test_demand_paging_segment_boundary() {
        // Test segment boundary check still works with demand paging
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        // Segment 8 has size 4000
        // Create a VA that exceeds the boundary
        // pw needs to be >= 4000
        // p=7, w=400 gives pw = 7*512 + 400 = 3984 (valid)
        // p=7, w=500 gives pw = 7*512 + 500 = 4084 (invalid)
        let va = VirtualAddress::from_raw((8 << 18) | (7 << 9) | 500);
        assert_eq!(va.pw, 4084);

        let result = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        assert_eq!(result, TranslationResult::SegmentBoundaryViolation);
    }

    #[test]
    fn test_demand_paging_invalid_segment() {
        // Test accessing non-existent segment
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        // Segment 5 doesn't exist
        let va = VirtualAddress::from_raw((5 << 18) | (0 << 9) | 0);

        let result = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        assert_eq!(result, TranslationResult::InvalidSegment);
    }

    #[test]
    fn test_demand_paging_frames_allocated_correctly() {
        // Verify that frames are allocated in the expected order
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();

        // Initially occupied: 3, 10, 13
        // Free frames start at: 2, 4, 5, 6, ...

        // VA1: No faults, no allocation
        let va1 = VirtualAddress::from_raw(2097162);
        translate_with_demand_paging(&va1, &mut pm, &disk, &mut ffl);

        // VA2: Page fault, allocates frame 2
        let va2 = VirtualAddress::from_raw(2097674);
        translate_with_demand_paging(&va2, &mut pm, &disk, &mut ffl);
        assert_eq!(pm.get_page_frame(3, 1), 2); // Page 1 of seg 8 now in frame 2

        // VA3: PT fault, allocates frame 4
        let va3 = VirtualAddress::from_raw(2359306);
        translate_with_demand_paging(&va3, &mut pm, &disk, &mut ffl);
        assert_eq!(pm.get_segment_pt_location(9), 4); // PT of seg 9 now in frame 4

        // VA4: Page fault, allocates frame 5
        let va4 = VirtualAddress::from_raw(2359818);
        translate_with_demand_paging(&va4, &mut pm, &disk, &mut ffl);
        assert_eq!(pm.get_page_frame(4, 1), 5); // Page 1 of seg 9 now in frame 5
    }

    // =========================================================================
    // Additional edge case tests
    // =========================================================================

    #[test]
    fn test_translate_zero_va() {
        // VA = 0 means s=0, p=0, w=0
        // Segment 0 doesn't exist (not initialized)
        let pm = crate::memory::PhysicalMemory::new();
        
        let va = VirtualAddress::from_raw(0);
        assert_eq!(va.s, 0);
        assert_eq!(va.p, 0);
        assert_eq!(va.w, 0);
        
        let result = translate(&va, &pm);
        assert_eq!(result, TranslationResult::InvalidSegment);
    }

    #[test]
    fn test_translate_max_valid_offset() {
        // Test with maximum valid offset (w = 511)
        let mut pm = crate::memory::PhysicalMemory::new();
        
        // Set up segment 1 with size 1024 (2 pages worth)
        pm.set_segment_entry(1, 1024, 3);
        pm.set_page_entry(3, 0, 10);
        pm.set_page_entry(3, 1, 11);
        
        // VA with w = 511 (max offset)
        let va = VirtualAddress::from_raw((1 << 18) | (0 << 9) | 511);
        assert_eq!(va.w, 511);
        
        let result = translate(&va, &pm);
        // PA = 10 * 512 + 511 = 5631
        assert_eq!(result, TranslationResult::Success(5631));
    }

    #[test]
    fn test_translate_repeated_same_va() {
        // Translating the same VA multiple times should give same result
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();
        
        let va = VirtualAddress::from_raw(2097162);
        
        let result1 = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        let result2 = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        let result3 = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        
        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
        assert_eq!(result1, TranslationResult::Success(5130));
    }

    #[test]
    fn test_translate_after_page_fault_is_cached() {
        // After a page fault, the page should be resident and no more faults
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();
        
        // First translation causes page fault
        let va = VirtualAddress::from_raw(2097674); // (8, 1, 10)
        let result1 = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        assert_eq!(result1, TranslationResult::Success(1034));
        
        // Page should now be resident (positive value)
        assert!(pm.get_page_frame(3, 1) > 0);
        
        // Second translation should work without fault
        // Same result, and no frames allocated
        let free_before = ffl.free_count();
        let result2 = translate_with_demand_paging(&va, &mut pm, &disk, &mut ffl);
        let free_after = ffl.free_count();
        
        assert_eq!(result2, TranslationResult::Success(1034));
        assert_eq!(free_before, free_after); // No new frame allocated
    }

    #[test]
    fn test_segment_size_boundary_exact() {
        // Test exactly at segment boundary
        let mut pm = crate::memory::PhysicalMemory::new();
        
        // Segment with size 100
        pm.set_segment_entry(2, 100, 5);
        pm.set_page_entry(5, 0, 10);
        
        // pw = 99 should work (size is 100, so 0-99 valid)
        let va_valid = VirtualAddress::from_raw((2 << 18) | (0 << 9) | 99);
        assert_eq!(va_valid.pw, 99);
        let result = translate(&va_valid, &pm);
        assert_eq!(result, TranslationResult::Success(10 * 512 + 99));
        
        // pw = 100 should fail
        let va_invalid = VirtualAddress::from_raw((2 << 18) | (0 << 9) | 100);
        assert_eq!(va_invalid.pw, 100);
        let result = translate(&va_invalid, &pm);
        assert_eq!(result, TranslationResult::SegmentBoundaryViolation);
    }

    #[test]
    fn test_multiple_segments_independent() {
        // Changes to one segment shouldn't affect another
        let mut pm = crate::memory::PhysicalMemory::new();
        let mut disk = crate::memory::Disk::new();
        let mut ffl = crate::memory::FreeFrameList::new();
        
        // Set up two independent segments
        pm.set_segment_entry(1, 1024, 3);
        pm.set_page_entry(3, 0, 10);
        
        pm.set_segment_entry(2, 2048, -5); // PT on disk
        disk.write(5, 0, 20);
        
        ffl.mark_occupied(3);
        ffl.mark_occupied(10);
        ffl.mark_occupied(20);
        
        // Translate in segment 1 (resident)
        let va1 = VirtualAddress::from_raw((1 << 18) | (0 << 9) | 100);
        let result1 = translate_with_demand_paging(&va1, &mut pm, &disk, &mut ffl);
        assert_eq!(result1, TranslationResult::Success(10 * 512 + 100));
        
        // Segment 2's PT should still be on disk
        assert!(pm.get_segment_pt_location(2) < 0);
        
        // Translate in segment 2 (causes PT fault)
        let va2 = VirtualAddress::from_raw((2 << 18) | (0 << 9) | 50);
        let result2 = translate_with_demand_paging(&va2, &mut pm, &disk, &mut ffl);
        assert_eq!(result2, TranslationResult::Success(20 * 512 + 50));
        
        // Segment 1 should be unaffected
        let result1_again = translate_with_demand_paging(&va1, &mut pm, &disk, &mut ffl);
        assert_eq!(result1_again, TranslationResult::Success(10 * 512 + 100));
    }

    #[test]
    fn test_large_batch_translation() {
        // Test translating many VAs at once
        let (mut pm, disk, mut ffl) = setup_demand_paging_memory();
        
        // Create a batch with repeated VAs
        let vas: Vec<u32> = (0..100)
            .map(|i| {
                match i % 4 {
                    0 => 2097162,
                    1 => 2097674,
                    2 => 2359306,
                    _ => 2359818,
                }
            })
            .collect();
        
        let results = translate_batch_with_demand_paging(&vas, &mut pm, &disk, &mut ffl);
        
        assert_eq!(results.len(), 100);
        
        // First 4 should establish the pattern
        assert_eq!(results[0], 5130);
        assert_eq!(results[1], 1034);
        assert_eq!(results[2], 6666);
        assert_eq!(results[3], 2570);
        
        // Subsequent ones should have same results (pages now resident)
        for i in 4..100 {
            let expected = match i % 4 {
                0 => 5130,
                1 => 1034,
                2 => 6666,
                _ => 2570,
            };
            assert_eq!(results[i], expected, "Mismatch at index {}", i);
        }
    }
}
