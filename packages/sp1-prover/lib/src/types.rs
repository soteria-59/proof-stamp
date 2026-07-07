use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String, // UUIDv7
    pub r#type: String, // EventType
    pub timestamp_unix_ms: u64,
    pub paragraph_index: u32,
    pub char_offset: u32,
    pub char_delta: i32, // e.g. +15 for insertions, -7 for deletions
    pub origin_rsid: Option<String>,
    pub foreign_style_detected: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParagraphSnapshot {
    pub index: u32,
    pub first_seen_ms: u64,
    pub last_edited_ms: u64,
    pub revision_count: u32,
    pub canonical_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub session_start_ms: u64,
    pub session_end_ms: u64,
    pub total_typed_chars: u32,
    pub total_pasted_chars: u32,
    pub total_ai_inserted_chars: u32,
    pub paste_event_count: u32,
    pub ai_insertion_count: u32,
    pub find_replace_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub protocol_version: String,
    pub document_id: String,
    pub device_id: String,
    pub device_pubkey: String, // hex encoded Ed25519 public key
    pub parent_cid: Option<String>,
    pub events: Vec<EventRecord>,
    pub snapshots: Vec<ParagraphSnapshot>,
    pub metrics: SessionMetrics,
    pub content_blocks: Vec<String>, // UTF-8 normalized paragraphs
    pub seal_timestamp_ms: u64,
    pub bundle_hash: String, // Computed over the canonical serialization of the bundle
    pub device_signature: String, // hex encoded Ed25519 signature of the bundle_hash
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicOutputs {
    pub doc_hash: [u8; 32],
    pub session_hash: [u8; 32],
    pub bundle_hash: [u8; 32],
    pub document_commitment: [u8; 32],
    pub device_pubkey: String,
    pub seal_timestamp_ms: u64,
    pub paste_event_count: u32,
    pub ai_insertion_count: u32,
    pub total_typed_chars: u32,
    pub parent_cid: Option<String>,
}

impl EvidenceBundle {
    /// Serializes the events array to a canonical JSON string
    /// This MUST strictly match the output of the TypeScript canonicalizer
    pub fn canonical_events_json(&self) -> String {
        serde_json::to_string(&self.events).expect("Failed to serialize events")
    }
}
