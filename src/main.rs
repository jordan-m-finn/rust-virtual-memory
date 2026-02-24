use std::env;
use std::process;

use vm_manager::io::{read_virtual_addresses, write_results, InitData};
use vm_manager::memory::{Disk, PhysicalMemory};
use vm_manager::translation::{translate_batch};

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

/// Main logic separated from main() for cleaner error handling
fn run(init_file: &str, input_file: &str, output_file: &str) -> Result<(), String> {
    // Step 1: Parse initialization file
    let init_data = InitData::from_file(init_file)?;

    // Step 2: Initialize physical memory and disk
    let mut pm = PhysicalMemory::new();
    let mut disk = Disk::new();
    init_data.apply(&mut pm, &mut disk);

    // Step 3: Read virtual addresses
    let vas = read_virtual_addresses(input_file)?;

    // Step 4: Translate each VA to PA
    let results = translate_batch(&vas, &pm);

    // Step 5: Write results to output file
    write_results(output_file, &results)?;

    Ok(())
}
