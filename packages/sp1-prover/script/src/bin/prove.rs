use sp1_sdk::{ProverClient, SP1Stdin};
use kako_lib::{EvidenceBundle, SessionMetrics};
use std::env;

/// The ELF (executable linkable format) file for the Succinct RISC-V zkVM.
pub const KAKO_ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

#[tokio::main]
async fn main() {
    // Generate a dummy EvidenceBundle for testing
    let bundle = EvidenceBundle {
        protocol_version: "kako/0.1".to_string(),
        document_id: "test-doc".to_string(),
        device_id: "test-device".to_string(),
        device_pubkey: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a".to_string(), // valid length hex
        parent_cid: None,
        events: vec![],
        snapshots: vec![],
        metrics: SessionMetrics {
            session_start_ms: 0,
            session_end_ms: 0,
            total_typed_chars: 0,
            total_pasted_chars: 0,
            total_ai_inserted_chars: 0,
            paste_event_count: 0,
            ai_insertion_count: 0,
            find_replace_count: 0,
        },
        content_blocks: vec![],
        seal_timestamp_ms: 0,
        bundle_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(), // valid length hex
        device_signature: "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b".to_string(),
    };

    let mut stdin = SP1Stdin::new();
    stdin.write(&bundle);

    let client = ProverClient::from_env();

    // The vk (verifying key) is needed downstream
    let (pk, vk) = client.setup(KAKO_ELF);

    println!("Starting proof generation...");
    // Create a core proof (for local testing, rather than Groth16)
    let proof = client
        .prove(&pk, &stdin)
        .run()
        .expect("Proof generation failed");

    println!("Successfully generated proof!");

    // Save proof to a file
    let proof_bytes = bincode::serialize(&proof).unwrap();
    std::fs::write("proof.bin", proof_bytes).unwrap();
    std::fs::write("vk.bin", vk.bytes32().as_bytes()).unwrap();

    println!("Proof and VK saved.");
}
