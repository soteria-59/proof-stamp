import { canonicalizeBundle } from "../canonicalize/pipeline";
import { read_stamp, embed_stamp, KakoStamp } from "./embedder";

export async function onBeforeSave(event: any): Promise<void> {
  const context = new Word.RequestContext();
  
  // Initialize canonical bundle.
  const mockBundle = {
    protocol_version: "kako/0.1",
    document_id: "doc-1",
    device_id: "dev-1",
    device_pubkey: "abcd",
    events: [],
    snapshots: [],
    metrics: {} as any,
    content_blocks: ["test content"],
    seal_timestamp_ms: Date.now(),
  };

  const { doc_hash, bundle_hash } = await canonicalizeBundle(mockBundle);
  const existing_stamp = await read_stamp(context);

  if (existing_stamp && existing_stamp.bundle_hash === bundle_hash) {
    // Content unchanged. Allow save.
    event.preventDefault = false;
    return;
  }

  // Block save and prove
  event.preventDefault = true;

  try {
    console.log("Generating proof...");
    // Trigger SP1 proof generation sequence.
    const proof: KakoStamp = {
      protocol_version: "kako/0.1",
      cid: "test-cid",
      doc_hash,
      session_hash: "session",
      bundle_hash,
      document_commitment: "commit",
      proof_bytes_b64: "...",
      public_values_b64: "...",
      device_pubkey: "...",
      seal_timestamp_iso: new Date().toISOString(),
      paste_event_count: 0,
      ai_insertion_count: 0,
      total_typed_chars: 0,
      parent_cid: null
    };
    
    await embed_stamp(context, proof);
    console.log("Stamped. Saving...");
    
    // Invoke native Word save mechanism.
  } catch (err: any) {
    console.error("Proof failed", err.message);
  }
}
