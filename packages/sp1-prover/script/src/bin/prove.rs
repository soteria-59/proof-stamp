use sp1_sdk::{ProverClient, SP1Stdin, HashableKey, Prover, ProvingKey};
use proofstamp_lib::EvidenceBundle;
use std::env;
use std::fs;

/// The ELF (executable linkable format) file for the Succinct RISC-V zkVM.
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

    // The vk (verifying key) is needed downstream
    let pk = client.setup(sp1_sdk::Elf::Static(PROOFSTAMP_ELF)).await.unwrap();
    let vk = pk.verifying_key();

    println!("Starting Proof Stamp generation...");
    // Create a core proof (for local testing, rather than Groth16)
    let proof = client
        .prove(&pk, stdin)
        .await
        .expect("Proof generation failed");

    println!("Successfully generated proof!");

    // Save proof to a file
    let proof_bytes = bincode::serialize(&proof).unwrap();
    std::fs::write("proof.bin", proof_bytes).unwrap();
    std::fs::write("vk.bin", vk.bytes32().as_bytes()).unwrap();

    println!("Proof and VK saved.");
}
