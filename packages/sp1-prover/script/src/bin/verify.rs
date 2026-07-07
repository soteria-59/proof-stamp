use sp1_sdk::{ProverClient, SP1ProofWithPublicValues};
use std::env;

pub const KAKO_ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

fn main() {
    let client = ProverClient::from_env();
    let (_, vk) = client.setup(KAKO_ELF);

    let proof_bytes = std::fs::read("proof.bin").expect("Could not read proof.bin");
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&proof_bytes).expect("Failed to deserialize proof");

    client.verify(&proof, &vk).expect("Verification failed");

    println!("Successfully verified proof!");
}
