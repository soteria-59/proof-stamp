use proofstamp_lib::{EvidenceBundle, EventRecord, ParagraphSnapshot, SessionMetrics};
use ed25519_dalek::{SigningKey, Signer};
use rand::rngs::OsRng;
use std::fs;

fn main() {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let pubkey = signing_key.verifying_key();
    
    let bundle_hash_bytes = [0u8; 32];
    let sig = signing_key.sign(&bundle_hash_bytes);

    let bundle = EvidenceBundle {
        protocol_version: "1.0".to_string(),
        document_id: "doc-123".to_string(),
        device_id: "dev-456".to_string(),
        device_pubkey: hex::encode(pubkey.as_bytes()),
        parent_cid: None,
        events: vec![],
        snapshots: vec![],
        metrics: SessionMetrics {
            session_start_ms: 1000,
            session_end_ms: 2000,
            total_typed_chars: 0,
            total_pasted_chars: 0,
            total_ai_inserted_chars: 0,
            paste_event_count: 0,
            ai_insertion_count: 0,
            find_replace_count: 0,
        },
        content_blocks: vec!["Hello World".to_string()],
        seal_timestamp_ms: 2000,
        bundle_hash: hex::encode(bundle_hash_bytes),
        device_signature: hex::encode(sig.to_bytes()),
    };

    let json = serde_json::to_string_pretty(&bundle).unwrap();
    fs::write("test_bundle.json", json).unwrap();
    println!("Successfully generated test_bundle.json!");
}
