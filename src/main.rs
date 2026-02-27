use std::env;
use std::process;

use rust_virtual_memory::io::{read_virtual_addresses, write_results, InitData};
use rust_virtual_memory::memory::{Disk, PhysicalMemory};
use rust_virtual_memory::translation::{translate_batch, translate_batch_with_demand_paging};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <init_file> <input_file> <output_file>", args[0]);
        process::exit(1);
    }

    if let Err(e) = run(&args[1], &args[2], &args[3]) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn needs_demand_paging(init_data: &InitData) -> bool {
    for &(_, _, f) in &init_data.st_entries {
        if f < 0 { return true; }
    }
    for &(_, _, f) in &init_data.pt_entries {
        if f < 0 { return true; }
    }
    false
}

fn run(init_file: &str, input_file: &str, output_file: &str) -> Result<(), String> {
    let init_data = InitData::from_file(init_file)?;
    let mut pm = PhysicalMemory::new();
    let mut disk = Disk::new();
    let mut ffl = init_data.apply(&mut pm, &mut disk);

    let vas = read_virtual_addresses(input_file)?;

    let results = if needs_demand_paging(&init_data) {
        translate_batch_with_demand_paging(&vas, &mut pm, &disk, &mut ffl)
    } else {
        translate_batch(&vas, &pm)
    };

    write_results(output_file, &results)?;
    Ok(())
}
