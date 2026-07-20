use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, Prover, ProvingKey};
use std::env;

pub const PROOFSTAMP_ELF: &[u8] = include_bytes!("../../../program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program");

#[tokio::main]
async fn main() {
    let client = ProverClient::from_env().await;

    // Setup the client with the ELF file.
    let pk = client.setup(sp1_sdk::Elf::Static(PROOFSTAMP_ELF)).await.unwrap();
    let vk = pk.verifying_key();

    let proof_bytes = std::fs::read("proof.bin").expect("Could not read proof.bin");
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&proof_bytes).expect("Failed to deserialize proof");

    // Verify the proof
    client
        .verify(&proof, &vk, None)
        .expect("Failed to verify proof");

    println!("Successfully verified proof!");
}
