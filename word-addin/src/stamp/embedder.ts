// embedder.ts
const KAKO_XMLNS = "http://kako.io/v1/stamp";

export interface KakoStamp {
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
  stamp: KakoStamp
): Promise<void> {
  // Remove existing stamp if present
  const parts = context.document.customXmlParts.getByNamespace(KAKO_XMLNS);
  await context.sync();
  
  parts.items.forEach(p => p.delete());

  // Write new stamp as XML
  const xml = stamp_to_xml(stamp);
  context.document.customXmlParts.add(xml);
  await context.sync();
}

export async function read_stamp(
  context: Word.RequestContext
): Promise<KakoStamp | null> {
  const parts = context.document.customXmlParts.getByNamespace(KAKO_XMLNS);
  await context.sync();
  
  if (parts.items.length === 0) return null;
  
  const xml = parts.items[0].getXml();
  await context.sync();
  
  return xml_to_stamp(xml.value);
}

function stamp_to_xml(stamp: KakoStamp): string {
  // Minimal serialization to XML
  return `<kako:stamp xmlns:kako="${KAKO_XMLNS}">
    ${Object.entries(stamp).map(([k, v]) => `<${k}>${v}</${k}>`).join("")}
  </kako:stamp>`;
}

function xml_to_stamp(xmlStr: string): KakoStamp {
  // Parse KakoStamp XML namespace.
  return {} as KakoStamp;
}
