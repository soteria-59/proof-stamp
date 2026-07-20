use sp1_sdk::{ProverClient, SP1Stdin, Prover};
use proofstamp_lib::EvidenceBundle;
use std::env;
use std::fs;

pub const PROOFSTAMP_ELF: &[u8] = include_bytes!("../../../program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program");

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <bundle.json>", args[0]);
        std::process::exit(1);
    }

    let bundle_path = &args[1];
    let bundle_json = fs::read_to_string(bundle_path)
        .expect("Failed to read evidence bundle JSON file");
    
    let bundle: EvidenceBundle = serde_json::from_str(&bundle_json)
        .expect("Failed to parse evidence bundle JSON");

    let mut stdin = SP1Stdin::new();
    stdin.write(&bundle);

    let client = ProverClient::from_env().await;

    // Execute the program
    println!("Executing Proof Stamp program to count cycles...");
    let (_output, report) = client.execute(sp1_sdk::Elf::Static(PROOFSTAMP_ELF), stdin).await.unwrap();
    
    println!("cycles: {}", report.total_instruction_count());
    println!("syscalls: {:?}", report.syscall_counts);

    // Print out the tracked cycles for each block
    println!("\nCycle Tracking Report:");
    for (name, count) in &report.cycle_tracker {
        println!("{}: {} cycles", name, count);
    }
}
