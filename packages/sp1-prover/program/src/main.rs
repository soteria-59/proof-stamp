#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Sha256, Digest};
use ed25519_dalek::{VerifyingKey, Signature, Verifier};
use kako_lib::{EvidenceBundle, PublicOutputs};

pub fn main() {
    // --- Private inputs ---
    let bundle: EvidenceBundle = sp1_zkvm::io::read::<EvidenceBundle>();

    // --- 1. Hash document content blocks ---
    let mut content_hasher = Sha256::new();
    for block in &bundle.content_blocks {
        content_hasher.update(block.as_bytes());
        content_hasher.update(b"\n\n");
    }
    let doc_hash: [u8; 32] = content_hasher.finalize().into();

    // --- 2. Hash session events ---
    let events_json = bundle.canonical_events_json();
    let session_hash: [u8; 32] = Sha256::digest(events_json.as_bytes()).into();

    // --- 3. Build document commitment ---
    let mut commitment_hasher = Sha256::new();
    commitment_hasher.update(&doc_hash);
    commitment_hasher.update(&session_hash);
    commitment_hasher.update(&bundle.seal_timestamp_ms.to_le_bytes());
    commitment_hasher.update(&(bundle.metrics.paste_event_count as u32).to_le_bytes());
    commitment_hasher.update(&(bundle.metrics.ai_insertion_count as u32).to_le_bytes());
    commitment_hasher.update(&(bundle.metrics.total_typed_chars as u32).to_le_bytes());
    if let Some(ref parent) = bundle.parent_cid {
        commitment_hasher.update(parent.as_bytes());
    } else {
        commitment_hasher.update(b"genesis");
    }
    let document_commitment: [u8; 32] = commitment_hasher.finalize().into();

    // --- 4. Verify device signature over bundle_hash ---
    let bundle_hash_bytes: [u8; 32] = hex::decode(&bundle.bundle_hash)
        .expect("invalid bundle hash hex")
        .try_into()
        .expect("bundle hash must be 32 bytes");

    let vk = VerifyingKey::from_bytes(
        &hex::decode(&bundle.device_pubkey).unwrap().try_into().unwrap()
    ).expect("invalid device pubkey");
    
    let sig = Signature::from_bytes(
        &hex::decode(&bundle.device_signature).unwrap().try_into().unwrap()
    );
    
    vk.verify(&bundle_hash_bytes, &sig).expect("device signature verification failed");

    // --- 5. Commit public outputs ---
    let out = PublicOutputs {
        doc_hash,
        session_hash,
        bundle_hash: bundle_hash_bytes,
        document_commitment,
        device_pubkey:     bundle.device_pubkey.clone(),
        seal_timestamp_ms: bundle.seal_timestamp_ms,
        paste_event_count: bundle.metrics.paste_event_count,
        ai_insertion_count: bundle.metrics.ai_insertion_count,
        total_typed_chars: bundle.metrics.total_typed_chars,
        parent_cid:        bundle.parent_cid.clone(),
    };
    sp1_zkvm::io::commit(&out);
}
