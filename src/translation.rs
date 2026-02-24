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

/// Translate a virtual address to a physical address
pub fn translate(_va: &VirtualAddress, _pm: &[i32]) -> TranslationResult {
    // TODO:- implement basic translation
    // TODO: - extend for demand paging
    todo!("Implement address translation")
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
}
