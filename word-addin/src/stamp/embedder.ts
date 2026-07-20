// embedder.ts
const PROOFSTAMP_XMLNS = "http://proofstamp.io/v1/stamp";

export interface ProofStamp {
  protocol_version: string;
  cid: string;
  doc_hash: string;
  session_hash: string;
  bundle_hash: string;
  document_commitment: string;
  proof_bytes_b64: string;
  public_values_b64: string;
  device_pubkey: string;
  seal_timestamp_iso: string;
  paste_event_count: number;
  ai_insertion_count: number;
  total_typed_chars: number;
  parent_cid: string | null;
}

export async function embed_stamp(
  context: Word.RequestContext,
  stamp: ProofStamp
): Promise<void> {
  // Remove existing stamp if present
  const parts = context.document.customXmlParts.getByNamespace(PROOFSTAMP_XMLNS);
  await context.sync();
  
  parts.items.forEach(p => p.delete());

  // Write new stamp as XML
  const xml = stamp_to_xml(stamp);
  context.document.customXmlParts.add(xml);
  await context.sync();
}

export async function read_stamp(
  context: Word.RequestContext
): Promise<ProofStamp | null> {
  const parts = context.document.customXmlParts.getByNamespace(PROOFSTAMP_XMLNS);
  await context.sync();
  
  if (parts.items.length === 0) return null;
  
  const xml = parts.items[0].getXml();
  await context.sync();
  
  return xml_to_stamp(xml.value);
}

function stamp_to_xml(stamp: ProofStamp): string {
  // Minimal serialization to XML
  return `<proofstamp:stamp xmlns:proofstamp="${PROOFSTAMP_XMLNS}">
    ${Object.entries(stamp).map(([k, v]) => `<${k}>${v}</${k}>`).join("")}
  </proofstamp:stamp>`;
}

function xml_to_stamp(xmlStr: string): ProofStamp {
  // Parse ProofStamp XML namespace.
  return {} as ProofStamp;
}
