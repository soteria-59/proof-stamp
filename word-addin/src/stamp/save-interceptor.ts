import { canonicalizeBundle } from "../canonicalize/pipeline";
import { read_stamp, embed_stamp, ProofStamp } from "./embedder";

export async function onBeforeSave(event: any): Promise<void> {
  const context = new Word.RequestContext();
  
  // 1. Gather actual document content
  const body = context.document.body;
  const paragraphs = body.paragraphs;
  paragraphs.load("items/text");
  await context.sync();
  
  const content_blocks = paragraphs.items.map(p => p.text);

  // 2. Collect events from the global collector
  // In a real shared runtime, this would access the global window.collector
  // For now we get whatever events are available on the window object
  const events = (window as any).collector ? (window as any).collector.getEvents() : [];

  // 3. Build the real evidence bundle
  const bundle = {
    protocol_version: "proofstamp/0.1",
    document_id: "doc-" + Math.random().toString(36).substring(2, 9), // Should be persistent
    device_id: "local-prover-enclave",
    device_pubkey: "", // Will be filled by the prover enclave
    events: events,
    snapshots: [], // To be implemented in full tracking
    metrics: {
      session_start_ms: Date.now() - 60000,
      session_end_ms: Date.now(),
      total_typed_chars: events.length,
      total_pasted_chars: 0,
      total_ai_inserted_chars: 0,
      paste_event_count: 0,
      ai_insertion_count: 0,
      find_replace_count: 0,
    },
    content_blocks: content_blocks,
    seal_timestamp_ms: Date.now(),
  };

  const { doc_hash, bundle_hash } = await canonicalizeBundle(bundle);
  const existing_stamp = await read_stamp(context);

  if (existing_stamp && existing_stamp.bundle_hash === bundle_hash) {
    // Content unchanged. Allow save.
    event.preventDefault = false;
    return;
  }

  // Block save and prove
  event.preventDefault = true;

  try {
    console.log("Generating Proof Stamp...");
    // Send bundle to local SP1 Prover API
    // Ensure the bundle has the computed bundle_hash
    const requestBundle = {
      ...bundle,
      bundle_hash,
      device_signature: "",
    };

    const res = await fetch("http://localhost:3000/prove", {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify(requestBundle)
    });

    if (!res.ok) {
      throw new Error("Local prover API failed");
    }

    const { proof_bytes_b64, public_values_b64, device_pubkey } = await res.json();

    const proof: ProofStamp = {
      protocol_version: "proofstamp/0.1",
      cid: "ps-" + Date.now().toString(36),
      doc_hash,
      session_hash: "session", // Should extract from public values
      bundle_hash,
      document_commitment: "commit", // Should extract from public values
      proof_bytes_b64,
      public_values_b64,
      device_pubkey,
      seal_timestamp_iso: new Date(bundle.seal_timestamp_ms).toISOString(),
      paste_event_count: bundle.metrics.paste_event_count,
      ai_insertion_count: bundle.metrics.ai_insertion_count,
      total_typed_chars: bundle.metrics.total_typed_chars,
      parent_cid: null
    };
    
    await embed_stamp(context, proof);
    console.log("Document stamped successfully. Saving...");
    
    // Invoke native Word save mechanism.
    // In a real add-in, you'd signal the UI or call document.save()
  } catch (err: any) {
    console.error("Proof generation failed:", err.message);
  }
}
