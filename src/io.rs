use std::fs;
use std::path::Path;

use crate::constants::*;
use crate::memory::{Disk, FreeFrameList, PhysicalMemory};

#[derive(Debug, Default)]
pub struct InitData {
    pub st_entries: Vec<(u32, i32, i32)>,
    pub pt_entries: Vec<(u32, u32, i32)>,
}

impl InitData {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read init file: {}", e))?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, String> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Err("Init file is empty".to_string());
        }

        let st_entries = Self::parse_st_line(lines[0])?;
        let pt_entries = if lines.len() > 1 {
            Self::parse_pt_line(lines[1])?
        } else {
            Vec::new()
        };

        Ok(InitData { st_entries, pt_entries })
    }

    fn parse_st_line(line: &str) -> Result<Vec<(u32, i32, i32)>, String> {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        if tokens.len() % 3 != 0 {
            return Err(format!("ST line has {} tokens, expected multiple of 3", tokens.len()));
        }

        let mut entries = Vec::new();
        for chunk in tokens.chunks(3) {
            let s: u32 = chunk[0].parse().map_err(|_| format!("Invalid segment number: {}", chunk[0]))?;
            let z: i32 = chunk[1].parse().map_err(|_| format!("Invalid segment size: {}", chunk[1]))?;
            let f: i32 = chunk[2].parse().map_err(|_| format!("Invalid frame/block: {}", chunk[2]))?;

            if s >= MAX_SEGMENTS as u32 {
                return Err(format!("Segment number {} exceeds max {}", s, MAX_SEGMENTS - 1));
            }
            entries.push((s, z, f));
        }
        Ok(entries)
    }

    fn parse_pt_line(line: &str) -> Result<Vec<(u32, u32, i32)>, String> {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        if tokens.len() % 3 != 0 {
            return Err(format!("PT line has {} tokens, expected multiple of 3", tokens.len()));
        }

        let mut entries = Vec::new();
        for chunk in tokens.chunks(3) {
            let s: u32 = chunk[0].parse().map_err(|_| format!("Invalid segment number: {}", chunk[0]))?;
            let p: u32 = chunk[1].parse().map_err(|_| format!("Invalid page number: {}", chunk[1]))?;
            let f: i32 = chunk[2].parse().map_err(|_| format!("Invalid frame/block: {}", chunk[2]))?;

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

    pub fn apply(&self, pm: &mut PhysicalMemory, disk: &mut Disk) -> FreeFrameList {
        let mut ffl = FreeFrameList::new();

        for &(segment, size, pt_location) in &self.st_entries {
            pm.set_segment_entry(segment, size, pt_location);
            if pt_location > 0 {
                ffl.mark_occupied(pt_location as u32);
            }
        }

        for &(segment, page, frame_location) in &self.pt_entries {
            let pt_location = pm.get_segment_pt_location(segment);
            if pt_location >= 0 {
                pm.set_page_entry(pt_location, page, frame_location);
            } else {
                let block = (-pt_location) as usize;
                disk.write(block, page as usize, frame_location);
            }
            if frame_location > 0 {
                ffl.mark_occupied(frame_location as u32);
            }
        }

        ffl
    }
}

pub fn read_virtual_addresses<P: AsRef<Path>>(path: P) -> Result<Vec<u32>, String> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|e| format!("Failed to read input file: {}", e))?;
    let mut addresses = Vec::new();
    for token in content.split_whitespace() {
        let va: u32 = token.parse().map_err(|_| format!("Invalid virtual address: {}", token))?;
        addresses.push(va);
    }
    Ok(addresses)
}

pub fn write_results<P: AsRef<Path>>(path: P, results: &[i32]) -> Result<(), String> {
    let output: Vec<String> = results.iter().map(|r| r.to_string()).collect();
    let content = output.join(" ");
    fs::write(path.as_ref(), content).map_err(|e| format!("Failed to write output file: {}", e))
}
