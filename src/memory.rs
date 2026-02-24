use crate::constants::*;

/// Physical Memory - simulates the main memory hardware
/// 
/// PM is organized as 1024 frames of 512 words each.
/// Frames 0 and 1 are reserved for the Segment Table.
pub struct PhysicalMemory {
    // TODO:implement memory storage
}

/// Paging Disk - simulates secondary storage for demand paging
///
/// Organized as 1024 blocks of 512 words each.
/// Non-resident pages and page tables are stored here.
pub struct Disk {
    // TODO:implement disk storage
}

/// Tracks which frames are available for allocation
///
/// Used during demand paging to find free frames when
/// loading pages or page tables from disk.
pub struct FreeFrameList {
    // TODO: implement free frame tracking
}
