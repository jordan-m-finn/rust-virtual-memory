use crate::constants::*;

/// Represents the decomposed components of a Virtual Address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualAddress {
    pub va: u32,
    pub s: u32,
    pub p: u32,
    pub w: u32,
    pub pw: u32,
}

impl VirtualAddress {
    /// Decompose a raw VA into its components
    pub fn from_raw(va: u32) -> Self {
        let s = va >> S_SHIFT;
        let p = (va >> P_SHIFT) & P_MASK;
        let w = va & W_MASK;
        let pw = va & PW_MASK;

        VirtualAddress { va, s, p, w, pw }
    }

    /// Compute the sp value (segment + page combined) for TLB lookups
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
    Success(i32),
    SegmentBoundaryViolation,
    InvalidSegment,
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
pub fn translate_batch(vas: &[u32], pm: &crate::memory::PhysicalMemory) -> Vec<i32> {
    vas.iter()
        .map(|&va| {
            let va = VirtualAddress::from_raw(va);
            translate(&va, pm).to_output()
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
        init.apply(&mut pm, &mut disk);

        // Now translate
        let vas = vec![1575424, 1575863, 1575864];
        let results = translate_batch(&vas, &pm);

        assert_eq!(results, vec![4608, 5047, -1]);
    }
}
