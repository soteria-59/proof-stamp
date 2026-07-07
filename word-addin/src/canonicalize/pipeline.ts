import { EventRecord, EventType, EvidenceBundle, ParagraphSnapshot } from "./schema";

/**
 * Normalizes text content for the canonicalization pipeline.
 * Ensures NFC, Unix line endings, and no trailing whitespace.
 */
export function normalizeContent(content: string): string {
  // 1. NFC Normalization
  let normalized = content.normalize("NFC");
  
  // 2. Line endings \r\n and \r -> \n
  normalized = normalized.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  
  // 3. Trailing whitespace per line: strip
  normalized = normalized.split("\n").map(line => line.trimEnd()).join("\n");
  
  // 4. Paragraph trailing newlines: normalize to single \n
  // Word doc parsing sometimes adds extra newlines at the end of paragraphs.
  normalized = normalized.replace(/\n+$/, "\n");
  
  return normalized;
}

/**
 * Creates a deterministic JSON representation of events.
 * Keys are sorted and whitespace is removed to ensure exact byte match with Rust.
 */
export function serializeEventsCanonical(events: EventRecord[]): string {
  // Enforce struct field order to guarantee deterministic JSON output matching the Rust prover.
  
  const orderedEvents = events.map(e => {
    return {
      id: e.id,
      type: e.type,
      timestamp_unix_ms: e.timestamp_unix_ms,
      paragraph_index: e.paragraph_index,
      char_offset: e.char_offset,
      char_delta: e.char_delta,
      ...(e.origin_rsid !== undefined && { origin_rsid: e.origin_rsid }),
      ...(e.foreign_style_detected !== undefined && { foreign_style_detected: e.foreign_style_detected })
    };
  });

  return JSON.stringify(orderedEvents);
}

/**
 * Compute SHA-256 using Web Crypto API.
 * Returns a hex string.
 */
export async function sha256(data: string | Uint8Array): Promise<string> {
  const buffer = typeof data === "string" ? new TextEncoder().encode(data) : data;
  const hashBuffer = await crypto.subtle.digest("SHA-256", buffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map(b => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Stage 5 & 6 of the Pipeline: Generate the hashes for the evidence bundle.
 */
export async function canonicalizeBundle(bundle: Omit<EvidenceBundle, 'bundle_hash' | 'device_signature'>): Promise<{ doc_hash: string, session_hash: string, bundle_hash: string }> {
  // Stage 5: Content commitment
  // SHA-256 of UTF-8 join of content_blocks with \n\n separator
  const contentString = bundle.content_blocks.join("\n\n");
  const doc_hash = await sha256(contentString);

  // Stage 6: Session commitment
  const eventsJson = serializeEventsCanonical(bundle.events);
  const session_hash = await sha256(eventsJson);

  // Stage 7: Bundle hash
  // Generate deterministic JSON representation of the bundle payload.
  const bundleJson = JSON.stringify({
    protocol_version: bundle.protocol_version,
    document_id: bundle.document_id,
    device_id: bundle.device_id,
    device_pubkey: bundle.device_pubkey,
    ...(bundle.parent_cid !== undefined && { parent_cid: bundle.parent_cid }),
    events: JSON.parse(eventsJson), // use the pre-canonicalized events
    snapshots: bundle.snapshots,
    metrics: bundle.metrics,
    content_blocks: bundle.content_blocks,
    seal_timestamp_ms: bundle.seal_timestamp_ms,
  });
  
  const bundle_hash = await sha256(bundleJson);

  return { doc_hash, session_hash, bundle_hash };
}
