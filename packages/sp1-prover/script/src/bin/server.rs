use axum::{
    routing::post,
    Router,
    Json,
    http::Method,
};
use tower_http::cors::{Any, CorsLayer};
use sp1_sdk::{ProverClient, SP1Stdin, Prover, ProvingKey, HashableKey};
use proofstamp_lib::EvidenceBundle;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{SigningKey, Signer};
use rand::rngs::OsRng;
use tokio::net::TcpListener;
use hex;

pub const PROOFSTAMP_ELF: &[u8] = include_bytes!("../../../program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/program");

#[derive(Serialize, Deserialize)]
pub struct ProveResponse {
    pub proof_bytes_b64: String,
    pub public_values_b64: String,
    pub vk_bytes_b64: String,
    pub device_pubkey: String,
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/prove", post(prove_handler))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {}", addr);
    
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn prove_handler(Json(mut bundle): Json<EvidenceBundle>) -> Json<ProveResponse> {
    println!("Received proof request for document: {}", bundle.document_id);
    
    // Save the real bundle to disk for local testing and profiling
    if let Ok(json) = serde_json::to_string_pretty(&bundle) {
        let _ = std::fs::write("latest_bundle.json", json);
        println!("Saved incoming bundle to latest_bundle.json for profiling!");
    }
    
    // --- 1. Sign the bundle hash (Local Prover Enclave signing) ---
    // The local server acts as the device enclave and maintains a persistent identity.
    let key_path = "device_key.bin";
    let signing_key = if std::path::Path::new(key_path).exists() {
        let bytes = std::fs::read(key_path).expect("Failed to read device_key.bin");
        let bytes_array: [u8; 32] = bytes.try_into().expect("Invalid key length");
        SigningKey::from_bytes(&bytes_array)
    } else {
        let mut csprng = OsRng;
        let new_key = SigningKey::generate(&mut csprng);
        std::fs::write(key_path, new_key.to_bytes()).expect("Failed to write device_key.bin");
        new_key
    };
    let pubkey = signing_key.verifying_key();
    
    // Parse the bundle_hash from the incoming bundle
    let mut bundle_hash_bytes = [0u8; 32];
    if !bundle.bundle_hash.is_empty() {
        if let Ok(decoded) = hex::decode(&bundle.bundle_hash) {
            if decoded.len() == 32 {
                bundle_hash_bytes.copy_from_slice(&decoded);
            }
        }
    }
    
    let sig = signing_key.sign(&bundle_hash_bytes);
    
    // Update the bundle with the valid signature
    bundle.device_pubkey = hex::encode(pubkey.as_bytes());
    bundle.device_signature = hex::encode(sig.to_bytes());

    // --- 2. Setup SP1 Prover ---
    let client = ProverClient::from_env().await;
    let pk = client.setup(sp1_sdk::Elf::Static(PROOFSTAMP_ELF)).await.unwrap();
    let vk = pk.verifying_key();

    let mut stdin = SP1Stdin::new();
    stdin.write(&bundle);

    println!("Starting cryptographic proof generation (This may take 30-60s locally)...");
    let proof = client
        .prove(&pk, stdin)
        .await
        .expect("Proof generation failed");
    
    println!("Proof generated successfully!");

    let proof_bytes = bincode::serialize(&proof).unwrap();
    let proof_b64 = general_purpose::STANDARD.encode(&proof_bytes);
    let public_values_b64 = general_purpose::STANDARD.encode(proof.public_values.as_slice());
    let vk_b64 = general_purpose::STANDARD.encode(vk.bytes32());

    Json(ProveResponse {
        proof_bytes_b64: proof_b64,
        public_values_b64,
        vk_bytes_b64: vk_b64,
        device_pubkey: bundle.device_pubkey,
    })
}
