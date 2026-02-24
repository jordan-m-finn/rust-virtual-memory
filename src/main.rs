//! VM Manager - Main Entry Point
//!
//! Usage: vm_manager [OPTIONS] <init_file> <input_file> <output_file>
//!
//! Arguments:
//!   init_file   - Initialization file defining ST and PT entries
//!   input_file  - File containing virtual addresses to translate
//!   output_file - File to write physical addresses (or -1 for errors)
//!
//! Options:
//!   -v, --verbose  Print detailed translation information
//!   -h, --help     Print help information

use std::env;
use std::process;

use vm_manager::io::{read_virtual_addresses, write_results, InitData};
use vm_manager::memory::{Disk, FreeFrameList, PhysicalMemory};
use vm_manager::translation::{
    translate, translate_batch, translate_with_demand_paging,
    translate_batch_with_demand_paging, VirtualAddress, TranslationResult,
};

/// Command-line configuration
struct Config {
    init_file: String,
    input_file: String,
    output_file: String,
    verbose: bool,
}

fn main() {
    let config = match parse_args() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Run the VM manager and handle any errors
    if let Err(e) = run(&config) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn print_help(program: &str) {
    eprintln!("Virtual Memory Manager - Translates virtual addresses to physical addresses");
    eprintln!();
    eprintln!("Usage: {} [OPTIONS] <init_file> <input_file> <output_file>", program);
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  init_file   - Initialization file with ST/PT definitions");
    eprintln!("  input_file  - File containing virtual addresses (space-separated)");
    eprintln!("  output_file - Output file for physical addresses");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -v, --verbose  Print detailed translation information");
    eprintln!("  -h, --help     Print this help message");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} init.txt input.txt output.txt", program);
    eprintln!("  {} -v init.txt input.txt output.txt", program);
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let mut verbose = false;
    let mut positional: Vec<&String> = Vec::new();

    for arg in &args[1..] {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help(program);
                process::exit(0);
            }
            "-v" | "--verbose" => {
                verbose = true;
            }
            _ if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}\nUse --help for usage information.", arg));
            }
            _ => {
                positional.push(arg);
            }
        }
    }

    if positional.len() != 3 {
        print_help(program);
        return Err(format!("\nError: Expected 3 arguments, got {}", positional.len()));
    }

    Ok(Config {
        init_file: positional[0].clone(),
        input_file: positional[1].clone(),
        output_file: positional[2].clone(),
        verbose,
    })
}

/// Check if demand paging is needed based on initialization data
///
/// Demand paging is needed if any ST or PT entry contains a negative value
/// (indicating the PT or page is on disk rather than in memory)
fn needs_demand_paging(init_data: &InitData) -> bool {
    // Check ST entries for negative frame numbers (PT on disk)
    for &(_, _, frame_or_block) in &init_data.st_entries {
        if frame_or_block < 0 {
            return true;
        }
    }

    // Check PT entries for negative frame numbers (page on disk)
    for &(_, _, frame_or_block) in &init_data.pt_entries {
        if frame_or_block < 0 {
            return true;
        }
    }

    false
}

/// Main logic separated from main() for cleaner error handling
fn run(config: &Config) -> Result<(), String> {
    // Step 1: Parse initialization file
    let init_data = InitData::from_file(&config.init_file)?;
    let use_demand_paging = needs_demand_paging(&init_data);

    if config.verbose {
        eprintln!("=== VM Manager ===");
        eprintln!("Init file:   {}", config.init_file);
        eprintln!("Input file:  {}", config.input_file);
        eprintln!("Output file: {}", config.output_file);
        eprintln!("Mode:        {}", if use_demand_paging { "Demand Paging" } else { "Basic" });
        eprintln!();
        eprintln!("Segment Table Entries: {}", init_data.st_entries.len());
        for &(s, size, loc) in &init_data.st_entries {
            if loc >= 0 {
                eprintln!("  Segment {}: size={}, PT in frame {}", s, size, loc);
            } else {
                eprintln!("  Segment {}: size={}, PT in disk block {}", s, size, -loc);
            }
        }
        eprintln!("Page Table Entries: {}", init_data.pt_entries.len());
        eprintln!();
    }

    // Step 2: Initialize physical memory and disk
    let mut pm = PhysicalMemory::new();
    let mut disk = Disk::new();
    let mut ffl = init_data.apply(&mut pm, &mut disk);

    if config.verbose {
        eprintln!("Free frames available: {}", ffl.free_count());
        eprintln!();
    }

    // Step 3: Read virtual addresses
    let vas = read_virtual_addresses(&config.input_file)?;

    if config.verbose {
        eprintln!("Virtual addresses to translate: {}", vas.len());
        eprintln!();
    }

    // Step 4: Translate each VA to PA
    let results = if use_demand_paging {
        if config.verbose {
            translate_verbose_demand_paging(&vas, &mut pm, &disk, &mut ffl)
        } else {
            translate_batch_with_demand_paging(&vas, &mut pm, &disk, &mut ffl)
        }
    } else {
        if config.verbose {
            translate_verbose_basic(&vas, &pm)
        } else {
            translate_batch(&vas, &pm)
        }
    };

    if config.verbose {
        eprintln!();
        eprintln!("=== Summary ===");
        let successes = results.iter().filter(|&&r| r >= 0).count();
        let failures = results.iter().filter(|&&r| r < 0).count();
        eprintln!("Successful translations: {}", successes);
        eprintln!("Failed translations: {}", failures);
        eprintln!();
    }

    // Step 5: Write results to output file
    write_results(&config.output_file, &results)?;

    if config.verbose {
        eprintln!("Results written to: {}", config.output_file);
    }

    Ok(())
}

/// Translate with verbose output (basic mode)
fn translate_verbose_basic(vas: &[u32], pm: &PhysicalMemory) -> Vec<i32> {
    vas.iter()
        .map(|&raw_va| {
            let va = VirtualAddress::from_raw(raw_va);
            let result = translate(&va, pm);
            
            eprintln!("VA {} (s={}, p={}, w={}, pw={}) -> {}", 
                raw_va, va.s, va.p, va.w, va.pw,
                match result {
                    TranslationResult::Success(pa) => format!("PA {}", pa),
                    TranslationResult::SegmentBoundaryViolation => "ERROR: Segment boundary violation".to_string(),
                    TranslationResult::InvalidSegment => "ERROR: Invalid segment".to_string(),
                    TranslationResult::InvalidPage => "ERROR: Invalid page".to_string(),
                }
            );
            
            result.to_output()
        })
        .collect()
}

/// Translate with verbose output (demand paging mode)
fn translate_verbose_demand_paging(
    vas: &[u32],
    pm: &mut PhysicalMemory,
    disk: &Disk,
    ffl: &mut FreeFrameList,
) -> Vec<i32> {
    vas.iter()
        .map(|&raw_va| {
            let va = VirtualAddress::from_raw(raw_va);
            
            // Check for page faults before translation
            let pt_loc = pm.get_segment_pt_location(va.s);
            let pt_fault = pt_loc < 0;
            
            let result = translate_with_demand_paging(&va, pm, disk, ffl);
            
            let page_fault = if !pt_fault && pt_loc > 0 {
                // Only check if PT was already resident
                let orig_page_loc = pm.get_page_frame(pt_loc, va.p);
                orig_page_loc < 0
            } else {
                false
            };
            
            let fault_info = match (pt_fault, page_fault) {
                (true, _) => " [PT fault]",
                (false, true) => " [Page fault]",
                _ => "",
            };
            
            eprintln!("VA {} (s={}, p={}, w={}) -> {}{}", 
                raw_va, va.s, va.p, va.w,
                match result {
                    TranslationResult::Success(pa) => format!("PA {}", pa),
                    TranslationResult::SegmentBoundaryViolation => "ERROR: Segment boundary violation".to_string(),
                    TranslationResult::InvalidSegment => "ERROR: Invalid segment".to_string(),
                    TranslationResult::InvalidPage => "ERROR: Invalid page".to_string(),
                },
                fault_info
            );
            
            result.to_output()
        })
        .collect()
}
