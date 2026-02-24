//! VM Manager - Main Entry Point
//!
//! Usage: vm_manager <init_file> <input_file> <output_file>
//!
//! Arguments:
//!   init_file   - Initialization file defining ST and PT entries
//!   input_file  - File containing virtual addresses to translate
//!   output_file - File to write physical addresses (or -1 for errors)

use std::env;
use std::process;

use vm_manager::io::{read_virtual_addresses, write_results, InitData};
use vm_manager::memory::{Disk, PhysicalMemory};
use vm_manager::translation::{translate_batch, translate_batch_with_demand_paging};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <init_file> <input_file> <output_file>", args[0]);
        eprintln!();
        eprintln!("Virtual Memory Manager - Translates virtual addresses to physical addresses");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  init_file   - Initialization file with ST/PT definitions");
        eprintln!("  input_file  - File containing virtual addresses (space-separated)");
        eprintln!("  output_file - Output file for physical addresses");
        process::exit(1);
    }

    let init_file = &args[1];
    let input_file = &args[2];
    let output_file = &args[3];

    // Run the VM manager and handle any errors
    if let Err(e) = run(init_file, input_file, output_file) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
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
fn run(init_file: &str, input_file: &str, output_file: &str) -> Result<(), String> {
    // Step 1: Parse initialization file
    let init_data = InitData::from_file(init_file)?;

    // Step 2: Initialize physical memory and disk
    let mut pm = PhysicalMemory::new();
    let mut disk = Disk::new();
    let mut ffl = init_data.apply(&mut pm, &mut disk);

    // Step 3: Read virtual addresses
    let vas = read_virtual_addresses(input_file)?;

    // Step 4: Translate each VA to PA
    // Use demand paging if any negative values in init data
    let results = if needs_demand_paging(&init_data) {
        translate_batch_with_demand_paging(&vas, &mut pm, &disk, &mut ffl)
    } else {
        translate_batch(&vas, &pm)
    };

    // Step 5: Write results to output file
    write_results(output_file, &results)?;

    Ok(())
}
