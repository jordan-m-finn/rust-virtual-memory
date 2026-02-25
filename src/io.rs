use std::fs;
use std::path::Path;

use crate::constants::*;
use crate::memory::{Disk, FreeFrameList, PhysicalMemory};

/// Parsed contents of the initialization file
#[derive(Debug, Default)]
pub struct InitData {
    /// Segment table entries: (segment_num, size, frame_or_block)
    pub st_entries: Vec<(u32, i32, i32)>,

    /// Page table entries: (segment_num, page_num, frame_or_block)
    pub pt_entries: Vec<(u32, u32, i32)>,
}

impl InitData {
    /// Parse an initialization file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read init file: {}", e))?;

        Self::parse(&content)
    }

    /// Parse initialization data from a string
    pub fn parse(content: &str) -> Result<Self, String> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Err("Init file is empty".to_string());
        }

        // Parse line 1: ST entries (s z f triples)
        let st_entries = Self::parse_st_line(lines[0])?;

        // Parse line 2: PT entries (s p f triples) - may be empty or missing
        let pt_entries = if lines.len() > 1 {
            Self::parse_pt_line(lines[1])?
        } else {
            Vec::new()
        };

        Ok(InitData {
            st_entries,
            pt_entries,
        })
    }

    /// Parse line 1 (ST entries): s1 z1 f1 s2 z2 f2 ...
    fn parse_st_line(line: &str) -> Result<Vec<(u32, i32, i32)>, String> {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        if tokens.len() % 3 != 0 {
            return Err(format!(
                "ST line has {} tokens, expected multiple of 3",
                tokens.len()
            ));
        }

        let mut entries = Vec::new();
        for chunk in tokens.chunks(3) {
            let s: u32 = chunk[0]
                .parse()
                .map_err(|_| format!("Invalid segment number: {}", chunk[0]))?;
            let z: i32 = chunk[1]
                .parse()
                .map_err(|_| format!("Invalid segment size: {}", chunk[1]))?;
            let f: i32 = chunk[2]
                .parse()
                .map_err(|_| format!("Invalid frame/block: {}", chunk[2]))?;

            if s >= MAX_SEGMENTS as u32 {
                return Err(format!("Segment number {} exceeds max {}", s, MAX_SEGMENTS - 1));
            }

            entries.push((s, z, f));
        }

        Ok(entries)
    }

    /// Parse line 2 (PT entries): s1 p1 f1 s2 p2 f2 ...
    fn parse_pt_line(line: &str) -> Result<Vec<(u32, u32, i32)>, String> {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        if tokens.len() % 3 != 0 {
            return Err(format!(
                "PT line has {} tokens, expected multiple of 3",
                tokens.len()
            ));
        }

        let mut entries = Vec::new();
        for chunk in tokens.chunks(3) {
            let s: u32 = chunk[0]
                .parse()
                .map_err(|_| format!("Invalid segment number: {}", chunk[0]))?;
            let p: u32 = chunk[1]
                .parse()
                .map_err(|_| format!("Invalid page number: {}", chunk[1]))?;
            let f: i32 = chunk[2]
                .parse()
                .map_err(|_| format!("Invalid frame/block: {}", chunk[2]))?;

            if s >= MAX_SEGMENTS as u32 {
                return Err(format!("Segment number {} exceeds max {}", s, MAX_SEGMENTS - 1));
            }
            if p >= PT_SIZE as u32 {
                return Err(format!("Page number {} exceeds max {}", p, PT_SIZE - 1));
            }

            entries.push((s, p, f));
        }

        Ok(entries)
    }

    /// Apply the parsed initialization data to physical memory
    pub fn apply(&self, pm: &mut PhysicalMemory, disk: &mut Disk) -> FreeFrameList {
        let mut ffl = FreeFrameList::new();

        // Step 1: Initialize ST entries
        for &(segment, size, pt_location) in &self.st_entries {
            pm.set_segment_entry(segment, size, pt_location);

            // If PT is resident (positive frame number), mark that frame as occupied
            if pt_location > 0 {
                ffl.mark_occupied(pt_location as u32);
            }
        }

        // Step 2: Initialize PT entries
        for &(segment, page, frame_location) in &self.pt_entries {
            let pt_location = pm.get_segment_pt_location(segment);

            if pt_location >= 0 {
                pm.set_page_entry(pt_location, page, frame_location);
            } else {
                let block = (-pt_location) as usize;
                disk.write(block, page as usize, frame_location);
            }

            // If page is resident (positive frame number), mark that frame as occupied
            if frame_location > 0 {
                ffl.mark_occupied(frame_location as u32);
            }
        }
        
        ffl  // Return the FreeFrameList!
    }
}

/// Read virtual addresses from an input file
pub fn read_virtual_addresses<P: AsRef<Path>>(path: P) -> Result<Vec<u32>, String> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|e| format!("Failed to read input file: {}", e))?;

    parse_virtual_addresses(&content)
}

/// Parse virtual addresses from a string
pub fn parse_virtual_addresses(content: &str) -> Result<Vec<u32>, String> {
    let mut addresses = Vec::new();

    for token in content.split_whitespace() {
        let va: u32 = token
            .parse()
            .map_err(|_| format!("Invalid virtual address: {}", token))?;
        addresses.push(va);
    }

    Ok(addresses)
}

/// Write translation results to an output file
pub fn write_results<P: AsRef<Path>>(path: P, results: &[i32]) -> Result<(), String> {
    let output: Vec<String> = results.iter().map(|r| r.to_string()).collect();
    let content = output.join(" ");

    fs::write(path.as_ref(), content).map_err(|e| format!("Failed to write output file: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_init() {
        // From the spec's simple test case
        let content = "6 3000 4\n6 5 9";
        let init = InitData::parse(content).unwrap();

        assert_eq!(init.st_entries.len(), 1);
        assert_eq!(init.st_entries[0], (6, 3000, 4));

        assert_eq!(init.pt_entries.len(), 1);
        assert_eq!(init.pt_entries[0], (6, 5, 9));
    }

    #[test]
    fn test_parse_demand_paging_init() {
        // From the spec's demand paging test case
        let content = "8 4000 3 9 5000 -7\n8 0 10 8 1 -20 9 0 13 9 1 -25";
        let init = InitData::parse(content).unwrap();

        assert_eq!(init.st_entries.len(), 2);
        assert_eq!(init.st_entries[0], (8, 4000, 3));   // PT in frame 3
        assert_eq!(init.st_entries[1], (9, 5000, -7));  // PT in disk block 7

        assert_eq!(init.pt_entries.len(), 4);
        assert_eq!(init.pt_entries[0], (8, 0, 10));   // Page in frame 10
        assert_eq!(init.pt_entries[1], (8, 1, -20));  // Page in disk block 20
        assert_eq!(init.pt_entries[2], (9, 0, 13));   // Page in frame 13
        assert_eq!(init.pt_entries[3], (9, 1, -25));  // Page in disk block 25
    }

    #[test]
    fn test_parse_empty_pt_line() {
        // Valid: ST entries but no PT entries
        let content = "6 3000 4\n";
        let init = InitData::parse(content).unwrap();

        assert_eq!(init.st_entries.len(), 1);
        assert_eq!(init.pt_entries.len(), 0);
    }

    #[test]
    fn test_parse_missing_pt_line() {
        // Valid: only ST line, no PT line at all
        let content = "6 3000 4";
        let init = InitData::parse(content).unwrap();

        assert_eq!(init.st_entries.len(), 1);
        assert_eq!(init.pt_entries.len(), 0);
    }

    #[test]
    fn test_parse_invalid_token_count() {
        // Invalid: not a multiple of 3
        let content = "6 3000\n";
        let result = InitData::parse(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("multiple of 3"));
    }

    #[test]
    fn test_parse_invalid_number() {
        let content = "6 abc 4\n";
        let result = InitData::parse(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid"));
    }

    #[test]
    fn test_parse_segment_out_of_range() {
        // Segment 512 is out of range (max is 511)
        let content = "512 3000 4\n";
        let result = InitData::parse(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds max"));
    }

    #[test]
    fn test_apply_simple() {
        let content = "6 3000 4\n6 5 9";
        let init = InitData::parse(content).unwrap();

        let mut pm = PhysicalMemory::new();
        let mut disk = Disk::new();
        init.apply(&mut pm, &mut disk);

        // Check ST entry
        assert_eq!(pm.get_segment_size(6), 3000);
        assert_eq!(pm.get_segment_pt_location(6), 4);

        // Check PT entry (PT is in frame 4)
        assert_eq!(pm.get_page_frame(4, 5), 9);
    }

    #[test]
    fn test_apply_demand_paging() {
        let content = "8 4000 3 9 5000 -7\n8 0 10 8 1 -20 9 0 13 9 1 -25";
        let init = InitData::parse(content).unwrap();

        let mut pm = PhysicalMemory::new();
        let mut disk = Disk::new();
        init.apply(&mut pm, &mut disk);

        // Check ST entries
        assert_eq!(pm.get_segment_size(8), 4000);
        assert_eq!(pm.get_segment_pt_location(8), 3);
        assert_eq!(pm.get_segment_size(9), 5000);
        assert_eq!(pm.get_segment_pt_location(9), -7);

        // Check PT entries for segment 8 (PT is resident in frame 3)
        assert_eq!(pm.get_page_frame(3, 0), 10);
        assert_eq!(pm.get_page_frame(3, 1), -20);

        // Check PT entries for segment 9 (PT is on disk block 7)
        assert_eq!(disk.read(7, 0), 13);
        assert_eq!(disk.read(7, 1), -25);
    }

    #[test]
    fn test_parse_virtual_addresses() {
        let content = "1575424 1575863 1575864";
        let vas = parse_virtual_addresses(content).unwrap();

        assert_eq!(vas.len(), 3);
        assert_eq!(vas[0], 1575424);
        assert_eq!(vas[1], 1575863);
        assert_eq!(vas[2], 1575864);
    }

    #[test]
    fn test_parse_virtual_addresses_multiline() {
        // Should handle multiple lines
        let content = "1575424\n1575863\n1575864";
        let vas = parse_virtual_addresses(content).unwrap();

        assert_eq!(vas.len(), 3);
    }

    #[test]
    fn test_parse_virtual_addresses_empty() {
        let content = "";
        let vas = parse_virtual_addresses(content).unwrap();
        assert_eq!(vas.len(), 0);
    }

    #[test]
    fn test_write_results() {
        use std::io::Read;

        let results = vec![4608, 5047, -1];
        let path = "/tmp/test_output.txt";

        write_results(path, &results).unwrap();

        let mut content = String::new();
        std::fs::File::open(path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert_eq!(content, "4608 5047 -1");

        // Cleanup
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_full_simple_scenario() {
        // End-to-end test matching the spec's simple test case
        let init_content = "6 3000 4\n6 5 9";
        let va_content = "1575424 1575863 1575864";

        let init = InitData::parse(init_content).unwrap();
        let vas = parse_virtual_addresses(va_content).unwrap();

        let mut pm = PhysicalMemory::new();
        let mut disk = Disk::new();
        init.apply(&mut pm, &mut disk);

        // Verify setup matches expectation
        assert_eq!(pm.get_segment_size(6), 3000);
        assert_eq!(pm.get_segment_pt_location(6), 4);
        assert_eq!(pm.get_page_frame(4, 5), 9);

        // Verify VAs parsed correctly
        assert_eq!(vas, vec![1575424, 1575863, 1575864]);
    }
}
