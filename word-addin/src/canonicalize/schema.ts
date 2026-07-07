export enum EventType {
  TYPED = "TYPED",
  AUTOCORRECT = "AUTOCORRECT",
  SUGGESTION_ACCEPT = "SUGGESTION_ACCEPT",
  PASTE_CLIPBOARD = "PASTE_CLIPBOARD",
  PASTE_DRAGDROP = "PASTE_DRAGDROP",
  AI_INSERTION = "AI_INSERTION",
  TEMPLATE_INSERT = "TEMPLATE_INSERT",
  FIND_REPLACE = "FIND_REPLACE",
  DELETE = "DELETE",
  SESSION_START = "SESSION_START",
  SESSION_END = "SESSION_END",
}

export interface EventRecord {
  id: string; // UUIDv7
  type: EventType;
  timestamp_unix_ms: number;
  paragraph_index: number;
  char_offset: number;
  char_delta: number; // e.g. +15 for insertions, -7 for deletions
  origin_rsid?: string;
  foreign_style_detected?: boolean;
}

export interface ParagraphSnapshot {
  index: number;
  first_seen_ms: number;
  last_edited_ms: number;
  revision_count: number;
  canonical_hash: string;
}

export interface SessionMetrics {
  session_start_ms: number;
  session_end_ms: number;
  total_typed_chars: number;
  total_pasted_chars: number;
  total_ai_inserted_chars: number;
  paste_event_count: number;
  ai_insertion_count: number;
  find_replace_count: number;
}

export interface EvidenceBundle {
  protocol_version: string;
  document_id: string;
  device_id: string;
  device_pubkey: string;
  parent_cid?: string | null;
  events: EventRecord[];
  snapshots: ParagraphSnapshot[];
  metrics: SessionMetrics;
  content_blocks: string[];
  seal_timestamp_ms: number;
  bundle_hash: string;
  device_signature: string; // Computed over canonical serialization of all fields preceding it
}
