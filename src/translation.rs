use crate::constants::*;
use crate::memory::{Disk, FreeFrameList, PhysicalMemory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualAddress {
    pub s: u32,
    pub p: u32,
    pub w: u32,
    pub pw: u32,
}

impl VirtualAddress {
    pub fn from_raw(va: u32) -> Self {
        let s = va >> S_SHIFT;
        let p = (va >> P_SHIFT) & P_MASK;
        let w = va & W_MASK;
        let pw = va & PW_MASK;
        VirtualAddress { s, p, w, pw }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationResult {
    Success(i32),
    SegmentBoundaryViolation,
    InvalidSegment,
    InvalidPage,
}

impl TranslationResult {
    pub fn to_output(&self) -> i32 {
        match self {
            TranslationResult::Success(pa) => *pa,
            _ => INVALID_ADDRESS,
        }
    }
}

pub fn translate(va: &VirtualAddress, pm: &PhysicalMemory) -> TranslationResult {
    let segment_size = pm.get_segment_size(va.s);
    let pt_location = pm.get_segment_pt_location(va.s);

    if segment_size == 0 && pt_location == 0 {
        return TranslationResult::InvalidSegment;
    }

    if va.pw >= segment_size as u32 {
        return TranslationResult::SegmentBoundaryViolation;
    }

    if pt_location <= 0 {
        return TranslationResult::InvalidSegment;
    }

    let page_frame = pm.get_page_frame(pt_location, va.p);

    if page_frame <= 0 {
        return TranslationResult::InvalidPage;
    }

    let pa = page_frame * PAGE_SIZE as i32 + va.w as i32;
    TranslationResult::Success(pa)
}

pub fn translate_batch(vas: &[u32], pm: &PhysicalMemory) -> Vec<i32> {
    vas.iter()
        .map(|&va| {
            let va = VirtualAddress::from_raw(va);
            translate(&va, pm).to_output()
        })
        .collect()
}

pub fn translate_with_demand_paging(
    va: &VirtualAddress,
    pm: &mut PhysicalMemory,
    disk: &Disk,
    ffl: &mut FreeFrameList,
) -> TranslationResult {
    let segment_size = pm.get_segment_size(va.s);
    let mut pt_location = pm.get_segment_pt_location(va.s);

    if segment_size == 0 && pt_location == 0 {
        return TranslationResult::InvalidSegment;
    }

    if va.pw >= segment_size as u32 {
        return TranslationResult::SegmentBoundaryViolation;
    }

    if pt_location < 0 {
        let disk_block = (-pt_location) as usize;
        let new_frame = match ffl.allocate() {
            Some(f) => f,
            None => return TranslationResult::InvalidSegment,
        };
        disk.load_pt_from_disk(disk_block, new_frame, pm);
        pm.set_segment_entry(va.s, segment_size, new_frame as i32);
        pt_location = new_frame as i32;
    }

    let mut page_frame = pm.get_page_frame(pt_location, va.p);

    if page_frame < 0 {
        let disk_block = (-page_frame) as usize;
        let new_frame = match ffl.allocate() {
            Some(f) => f,
            None => return TranslationResult::InvalidPage,
        };
        disk.load_page_from_disk(disk_block, new_frame, pm);
        pm.set_page_entry(pt_location, va.p, new_frame as i32);
        page_frame = new_frame as i32;
    }

    if page_frame == 0 {
        return TranslationResult::InvalidPage;
    }

    let pa = page_frame * PAGE_SIZE as i32 + va.w as i32;
    TranslationResult::Success(pa)
}

pub fn translate_batch_with_demand_paging(
    vas: &[u32],
    pm: &mut PhysicalMemory,
    disk: &Disk,
    ffl: &mut FreeFrameList,
) -> Vec<i32> {
    vas.iter()
        .map(|&va| {
            let va = VirtualAddress::from_raw(va);
            translate_with_demand_paging(&va, pm, disk, ffl).to_output()
        })
        .collect()
}
