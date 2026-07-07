# Kako — Technical Specification
**v0.1-draft** | SP1 V6 (Hypercube) · SHA-256 · Ed25519 · Multi-chain

---

## 1. System Overview

Kako is a document provenance protocol. The Word add-in monitors a writing session, builds a tamper-evident evidence bundle, and generates an SP1 ZK proof on save. The proof is embedded into the document's XML properties layer — invisible in normal editing, immutable without the add-in, and verifiable on any supported chain without trusting Kako's servers.

**Invariants the protocol enforces:**
- A document cannot be saved without a valid proof covering the current content state.
- The proof is not in the document body or footer. It lives in `docProps/custom.xml` via the CustomXmlPart API, inaccessible from Word's editing UI.
- Any content change after stamping invalidates the embedded proof. The add-in detects this on every open and every save attempt.
- The proof is chain-agnostic. The Groth16 proof bytes are identical regardless of which chain verifies them. Chain-specific verifier contracts are deployed separately.

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  WORD ADD-IN  (TypeScript · Word JS API · Office.js)            │
│                                                                 │
│  EventCollector         Classifier          SessionManager      │
│  ─────────────          ──────────          ──────────────      │
│  onContentChanged   →  event typing     →  bundle accumulator   │
│  onBeforePaste         TYPED / PASTE /      offline queue       │
│  onParagraphAdded      AI_INSERTION /       session timeline    │
│  Copilot detection     FIND_REPLACE                             │
│           │                                                     │
│           ▼                                                     │
│  CanonPipeline                                                  │
│  ─────────────                                                  │
│  6-stage deterministic pipeline → EvidenceBundle               │
│           │                                                     │
│           ▼                                                     │
│  SaveInterceptor  ←── DocumentBeforeSave event                  │
│  ─────────────────                                              │
│  If no proof OR content changed since last proof:               │
│    → block save, invoke Prover, await proof                     │
│  If proof valid for current content:                            │
│    → allow save immediately                                     │
│           │                                                     │
│           ▼                                                     │
│  ProofEmbedder                                                  │
│  ─────────────                                                  │
│  Writes KakoStamp to CustomXmlPart (docProps/custom.xml)        │
│  Hidden ContentControl — cannotDelete=true, cannotEdit=true     │
│  PDF export: XMP metadata via post-save backend pipeline        │
└─────────────────────┬───────────────────────────────────────────┘
                      │ EvidenceBundle (JSON, private)
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│  KAKO PROVER SERVICE  (Rust · SP1 V6 Hypercube)                 │
│                                                                 │
│  Input (private):   EvidenceBundle                              │
│  Input (private):   device_signing_key (Ed25519)                │
│                                                                 │
│  Program (RISC-V, proved by SP1):                               │
│    1. sha256_precompile(content_blocks)  → doc_hash             │
│    2. sha256_precompile(event_records)   → session_hash         │
│    3. ed25519_verify(device_pubkey,                             │
│                      doc_hash ‖ session_hash ‖ timestamp,       │
│                      device_sig)         → assert true          │
│    4. commitment = sha256(doc_hash ‖ session_hash ‖             │
│                           timestamp ‖ paste_count ‖             │
│                           ai_count ‖ parent_cid)                │
│    5. commit(PublicOutputs)                                     │
│                                                                 │
│  Proof type: Groth16 (on-chain) or Compressed (off-chain)       │
│  Output: ~260 byte Groth16 proof + PublicOutputs                │
└─────────────────────┬───────────────────────────────────────────┘
                      │ proof + public_values
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│  CHAIN REGISTRY  (multi-chain, chain-agnostic proof)            │
│                                                                 │
│  EVM (Ethereum / L2):   SP1VerifierGateway.sol (Groth16)       │
│                          KakoRegistry.sol                       │
│  Solana:                 sp1-solana (Groth16 + BN254 precomp)  │
│                          ~280K compute units per verification   │
│  Sui:                    SP1 verifier (Soundness Labs)          │
│  Custom chain (v0.3):    Deploy verifier + registry             │
│                                                                 │
│  All chains store: cid → (doc_hash, proof_hash,                 │
│                            device_pubkey, timestamp, parent_cid)│
└─────────────────────┬───────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────────┐
│  VERIFIER  (Next.js + Rust verification binary)                 │
│                                                                 │
│  1. Extract KakoStamp from CustomXmlPart                        │
│  2. Re-canonicalize document → doc_hash_current                 │
│  3. doc_hash_current === KakoStamp.doc_hash  → integrity check  │
│  4. Verify Groth16 proof locally (no chain needed for content)  │
│  5. Registry lookup by CID → chain-anchored status              │
│  6. Render ProvenanceReport                                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Document Locking & Save Interception

### 3.1 Save interception

The add-in hooks `context.document.onBeforeSave` (Word JS API, available in Microsoft 365 / Word 2021+). This event fires before the document writes to disk and before the file format conversion step.

```typescript
// stamp/save-interceptor.ts

async function onBeforeSave(event: Word.SaveEventArgs): Promise<void> {
  const current_hash = await canonicalize_and_hash(context);
  const existing_stamp = await read_stamp(context);

  if (existing_stamp && existing_stamp.doc_hash === current_hash) {
    // Content unchanged since last proof. Allow save.
    event.preventDefault = false;
    return;
  }

  // Content changed or no stamp exists. Block save and prove.
  event.preventDefault = true;

  try {
    show_taskpane_status("Generating proof…");
    const bundle   = await build_evidence_bundle(context);
    const proof    = await request_proof(bundle);          // → ProverService
    await embed_stamp(context, proof);
    show_taskpane_status("Stamped. Saving…");
    await context.document.save();                         // Re-trigger save, stamp now valid
  } catch (err) {
    show_taskpane_status(`Proof failed: ${err.message}. Cannot save.`, "error");
    // Document remains unsaved. User must retry.
  }
}
```

`event.preventDefault = true` prevents the document from saving until the proof is embedded. The document is not saveable in any format (Ctrl+S, File > Save, AutoSave, SharePoint sync) without a valid proof covering the current content state.

### 3.2 Proof storage — not the footer

The KakoStamp is stored in `docProps/custom.xml` via the Office.js `CustomXmlParts` API. This namespace is invisible in Word's editing UI. Users cannot reach it with the cursor, cannot select it, cannot delete it from the document body.

```typescript
// stamp/embedder.ts

const KAKO_XMLNS = "http://kako.io/v1/stamp";

async function embed_stamp(
  context: Word.RequestContext,
  stamp: KakoStamp
): Promise<void> {
  // Remove existing stamp if present
  const parts = context.document.customXmlParts.getByNamespace(KAKO_XMLNS);
  await context.sync();
  parts.items.forEach(p => p.delete());

  // Write new stamp as XML
  const xml = stamp_to_xml(stamp);  // serializes KakoStamp struct to XML
  context.document.customXmlParts.add(xml);
  await context.sync();
}

async function read_stamp(
  context: Word.RequestContext
): Promise<KakoStamp | null> {
  const parts = context.document.customXmlParts.getByNamespace(KAKO_XMLNS);
  await context.sync();
  if (parts.items.length === 0) return null;
  const xml = parts.items[0].getXml();
  await context.sync();
  return xml_to_stamp(xml.value);
}
```

No footer. No ContentControl in the body. The stamp is in the XML layer of the `.docx` ZIP archive (`word/customXml/item1.xml`), which persists through Save As, email attachment, and SharePoint upload.

### 3.3 Tamper detection on open

```typescript
// stamp/extractor.ts

async function on_document_open(context: Word.RequestContext): Promise<void> {
  const stamp = await read_stamp(context);
  if (!stamp) {
    show_taskpane_status("Unstamped document. Will stamp on save.", "warn");
    return;
  }

  const current_hash = await canonicalize_and_hash(context);

  if (current_hash !== stamp.doc_hash) {
    show_taskpane_status(
      "CONTENT MISMATCH. Document was modified outside Kako. Current proof is invalid.",
      "error"
    );
    // Flag in session state — next save will force new proof generation
    session.proof_invalidated = true;
    return;
  }

  show_taskpane_status(`Stamped · ${stamp.cid.slice(0, 8)}… · Valid`, "ok");
}
```

Tamper detection is entirely local. No network call. The SHA-256 hash of the canonicalized document body is recomputed and compared against `stamp.doc_hash` every time the document is opened.

### 3.4 PDF export

When the user exports to PDF (File > Export > PDF), the add-in hooks `context.document.onAfterSave` and detects that the save format is PDF. It then calls the Kako backend to post-process the PDF file:

```typescript
// stamp/pdf-embedder.ts

// Backend route: POST /api/embed-pdf-xmp
// Body: { pdf_base64, stamp: KakoStamp }
// Returns: pdf_with_xmp_base64
// Uses: Apache PDFBox (Java) or pypdf (Python) to inject XMP metadata block
// XMP namespace: http://kako.io/v1/xmp
// Fields: kako:cid, kako:doc_hash, kako:proof_bytes_b64, kako:timestamp
```

The XMP metadata block in the PDF is not user-editable through Acrobat Reader or any standard PDF viewer. Adobe Acrobat Pro can edit it, which is a known limitation — same as any signed PDF's metadata. The primary trust anchor is the ZK proof itself, not the XMP field.

### 3.5 Word document file protection (post-stamp)

After a proof is generated and embedded, the add-in applies Word's built-in document editing restriction to prevent accidental saves that bypass the add-in. This is a secondary defence only — it can be bypassed by a user who disables the restriction:

```typescript
// NOT a security control — UX guardrail only
context.document.body.getRange().insertContentControl();
// Primary security is the hash mismatch detection on next open
```

The real tamper detection is the hash mismatch check. The file restriction is just to stop accidental edits.

---

## 4. Evidence Model

### 4.1 Event types

```typescript
// protocol/events.ts

export const EventType = {
  TYPED:              "TYPED",
  AUTOCORRECT:        "AUTOCORRECT",      // Reclassified as TYPED at pipeline stage 2
  SUGGESTION_ACCEPT:  "SUGGESTION_ACCEPT",// Reclassified as TYPED
  PASTE_CLIPBOARD:    "PASTE_CLIPBOARD",
  PASTE_DRAGDROP:     "PASTE_DRAGDROP",
  AI_INSERTION:       "AI_INSERTION",     // Copilot / inline AI completions
  TEMPLATE_INSERT:    "TEMPLATE_INSERT",  // Quick Parts, building blocks
  FIND_REPLACE:       "FIND_REPLACE",
  DELETE:             "DELETE",
  SESSION_START:      "SESSION_START",
  SESSION_END:        "SESSION_END",
} as const;
```

**AI_INSERTION detection:** Microsoft Copilot inserts content via `context.document.insertInline()` which does NOT fire `BeforePaste`. Detection requires hooking `Word.EventType.contentControlOnEnter` and comparing insertion source metadata in `Word.InsertFileOptions`. Copilot-sourced insertions carry a distinct `insertionOrigin` flag in the `onContentChanged` event payload. This must be implemented and tested against all current Copilot surfaces (inline suggestions, Copilot sidebar, Copilot rewrite). A Copilot-authored document with no TYPED events must not receive a clean stamp.

**FIND_REPLACE:** Classified separately. Reported in provenance report as operation log (term replaced, scope, timestamp). Not flagged as suspicious. Content delta reflected in final `doc_hash`.

### 4.2 Evidence schema

```typescript
// protocol/schema.ts

export interface EventRecord {
  id:               string;       // UUIDv4
  type:             EventType;
  timestamp_ms:     number;       // Unix ms, device clock (TEE-attested)
  paragraph_index:  number;
  char_offset:      number;
  char_delta:       number;       // positive = insert, negative = delete
  source_rsid?:     string;       // Word revision save ID of paste source
  style_foreign?:   boolean;      // true if pasted paragraph spacing differs from doc style
}

export interface ParagraphSnapshot {
  index:            number;
  first_seen_ms:    number;
  last_edited_ms:   number;
  revision_count:   number;
  content_hash:     string;       // SHA-256 of UTF-8 paragraph text (normalized)
}

export interface SessionMetrics {
  session_start_ms:         number;
  session_end_ms:           number;
  total_typed_chars:        number;
  total_pasted_chars:       number;
  total_ai_inserted_chars:  number;
  paste_event_count:        number;
  ai_insertion_count:       number;
  find_replace_count:       number;
  avg_typing_wpm:           number;
  velocity_p50:             number;  // chars/min, 50th percentile of active periods
  velocity_p95:             number;  // chars/min, 95th percentile — detect burst anomalies
}

export interface EvidenceBundle {
  protocol_version:   "kako/0.1";
  document_id:        string;     // Stable GUID from CustomXmlPart, set on first stamp
  device_id:          string;     // Stable device identifier
  device_pubkey:      string;     // Ed25519 public key, hex
  parent_cid?:        string;     // Previous proof CID (null for genesis stamp)
  events:             EventRecord[];
  snapshots:          ParagraphSnapshot[];
  metrics:            SessionMetrics;
  content_blocks:     string[];   // UTF-8 paragraphs, normalized, at seal time
  seal_timestamp_ms:  number;
  device_signature:   string;     // Ed25519 sig over sha256(bundle_bytes_without_sig)
}
```

---

## 5. Canonicalization Pipeline

Runs in TypeScript before `EvidenceBundle` is sent to the prover. Must be deterministic across Word versions, operating systems, and locales.

```
Stage 1 — Deduplication
  Collapse duplicate events within a 50ms window at the same char_offset.
  Word fires duplicate onContentChanged events during undo/redo replay.

Stage 2 — Classification
  AUTOCORRECT, SUGGESTION_ACCEPT → reclassify to TYPED.
  PASTE_DRAGDROP → reclassify to PASTE_CLIPBOARD.
  AI_INSERTION → keep as AI_INSERTION (never reclassify).
  FIND_REPLACE batch → collapse N individual edits into one FIND_REPLACE record
    when >3 identical-delta edits fire within 200ms. Store operation metadata.

Stage 3 — Content normalization
  Unicode: normalize to NFC.
  Line endings: \r\n and \r → \n.
  Trailing whitespace per line: strip.
  Paragraph trailing newlines: normalize to single \n.
  Strip Word internal markup: revision IDs (rsid), comment anchors,
    tracked change runs, smart tag markup.
  Heading prefix: prepend "H1:", "H2:", "H3:", "BODY:", "LIST:" per paragraph type.
    Heading level is authored structure, included in hash.
  Empty paragraphs: retain. Do not strip. They are authored.
  Font and style names: excluded from content_blocks. Not part of content.

Stage 4 — Paragraph segmentation
  Split normalized document text into paragraph array at seal time.
  Record ParagraphSnapshot for each paragraph.

Stage 5 — Content commitment
  content_blocks = [paragraph_0_text, paragraph_1_text, …]
  doc_hash = SHA-256( UTF-8 join of content_blocks with \n\n separator )

Stage 6 — Session commitment
  Serialize events array to canonical JSON (keys sorted, no whitespace).
  session_hash = SHA-256( canonical_events_json_bytes )
```

**Canonicalization invariants (must hold across implementations):**
- Unicode NFC before any hash operation
- Always `\n`, never `\r\n`
- Empty paragraphs included
- Heading prefix always present, colon-delimited
- Events JSON: sorted keys, no extra whitespace, UTF-8 bytes
- All hashes are raw SHA-256 (32 bytes), hex-encoded for storage

---

## 6. SP1 Prover

### 6.1 SP1 V6 (Hypercube) — what changed from V5

SP1 V6 introduces the Hypercube proof system, replacing the previous STARK architecture at the inner proving layer. Key differences that affect Kako:

| Property | SP1 V5 | SP1 V6 Hypercube |
|---|---|---|
| Inner proof system | STARKs (univariate) | Multilinear STARKs (Hypercube) |
| Commitment scheme | FRI (univariate) | Jagged PCS + Dense PCS (multilinear) |
| Shard protocol | Fixed shard sizes | Variable-length Jagged shards |
| LogUp | Univariate | LogUp GKR (more efficient) |
| Outer proof types | Core / Compressed / Groth16 / PLONK | Same (unchanged) |
| Field | BabyBear (2^31 − 2^27 + 1) | BabyBear (unchanged) |
| Migration | — | See upgrade guide at docs.succinct.xyz |

For Kako, the change is transparent — we write the same Rust program. SP1 V6 proves it faster.

### 6.2 Field

SP1 operates over **BabyBear** (prime p = 2^31 − 2^27 + 1 = 2,013,265,921). Not Mersenne31. BabyBear is optimized for SIMD-friendly 32-bit arithmetic. The field choice is SP1-internal — application code does not interact with field elements directly.

### 6.3 Hash function choice — SHA-256 via precompile, not Poseidon2

**Poseidon2 has no SP1 precompile.** It would run as raw RISC-V instructions at full cycle cost. SHA-256 has a patched SP1 precompile (`sha2` crate via `sp1-patches/RustCrypto-hashes`) that executes through a specialized high-performance circuit, reducing cycle count by orders of magnitude.

SHA-256 is also the correct choice for chain-agnostic verification — EVM, Solana, and Sui all have native SHA-256 support. Poseidon2 output would require a Poseidon2 verifier on each chain.

```toml
# sp1-prover/program/Cargo.toml

[dependencies]
sha2 = { git = "https://github.com/sp1-patches/RustCrypto-hashes",
          package = "sha2",
          tag = "patch-sha2-0.10.9-sp1-6.0.0" }

curve25519-dalek = { git = "https://github.com/sp1-patches/curve25519-dalek",
                      tag = "patch-4.1.3-sp1-6.0.0" }

sp1-zkvm = { version = "4.0.0" }
```

### 6.4 Signature scheme — Ed25519 via precompile

Ed25519 (curve25519-dalek) has a patched SP1 precompile. Verification runs through native SIMD-optimized circuit rather than RISC-V emulated field arithmetic. This is the dominant constraint cost in the circuit — the precompile is necessary.

Device attestation uses Ed25519 rather than secp256k1 for three reasons:
- curve25519-dalek patch is simpler to apply than k256/secp256k1 patch
- Ed25519 is natively supported on iOS/Android Secure Enclave and most TEE implementations
- Verification is faster and cheaper in-circuit than ECDSA over secp256k1

### 6.5 Circuit — `program/src/main.rs`

```rust
#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Sha256, Digest};
use ed25519_dalek::{VerifyingKey, Signature, Verifier};
use kako_lib::{EvidenceBundle, PublicOutputs};

pub fn main() {
    // --- Private inputs ---
    let bundle: EvidenceBundle = sp1_zkvm::io::read::<EvidenceBundle>();

    // --- 1. Hash document content blocks ---
    // content_blocks: Vec<String>, normalized paragraphs at seal time
    let mut content_hasher = Sha256::new();
    for block in &bundle.content_blocks {
        content_hasher.update(block.as_bytes());
        content_hasher.update(b"\n\n");
    }
    let doc_hash: [u8; 32] = content_hasher.finalize().into();

    // --- 2. Hash session events ---
    // canonical_events_json: sorted-key JSON of Vec<EventRecord>
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

    // --- 4. Verify device signature ---
    // Ed25519 signature over document_commitment, signed by device TEE key
    // curve25519-dalek patch routes this through SP1's Ed25519 precompile
    let vk = VerifyingKey::from_bytes(
        &hex::decode(&bundle.device_pubkey).unwrap().try_into().unwrap()
    ).expect("invalid device pubkey");
    let sig = Signature::from_bytes(
        &hex::decode(&bundle.device_signature).unwrap().try_into().unwrap()
    );
    vk.verify(&document_commitment, &sig)
      .expect("device signature verification failed");

    // --- 5. Commit public outputs ---
    let out = PublicOutputs {
        doc_hash,
        session_hash,
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
```

### 6.6 Proof types and selection

| Type | Size | On-chain gas | Trusted setup | When to use |
|---|---|---|---|---|
| Core | Proportional to execution | Not suitable on-chain | None | Local dev / testing only |
| Compressed | Constant (small STARK) | Not suitable on-chain | None | Off-chain verification, recursive aggregation |
| Groth16 | ~260 bytes | ~270k EVM gas · ~280K Solana CU | Aztec Ignition ceremony + Succinct entropy | **Production on-chain** |
| PLONK | ~868 bytes | ~300k EVM gas | Reuses Aztec Ignition, no new ceremony | Alternative if Groth16 ceremony assumptions unacceptable |

**Kako uses:**
- `Groth16` for on-chain anchoring (EVM, Solana, Sui)
- `Compressed` for local tamper detection (verifier-api, offline)

```rust
// prove.rs — proof generation script

let client = ProverClient::from_env().await;
let (pk, vk) = client.setup(KAKO_ELF);
let mut stdin = SP1Stdin::new();
stdin.write(&evidence_bundle);

// Development
let proof = client.prove(&pk, &stdin).run().await?;

// Production on-chain
let proof = client.prove(&pk, &stdin).groth16().run().await?;
// Returns ~260 byte proof usable on EVM, Solana, Sui
```

### 6.7 Prover network

```rust
// .env
SP1_PROVER=network
PROVER_NETWORK_RPC=https://rpc.succinct.xyz
SP1_PRIVATE_KEY=<PROVE_token_funded_key>
```

Succinct's prover network dispatches proving jobs to GPU/FPGA nodes. For Kako's circuit (small — SHA-256 hash of document + Ed25519 verify), typical proving time on the network is **< 5 seconds** end-to-end. Provers include GPU clusters (RTX 4090, RTX 5090) and FPGA accelerators (ZAN team: 20x over CPU). The circuit is embarrassingly small relative to what the network handles (Ethereum block proving).

Hardware acceleration is transparent to the SP1 API — the network handles GPU/FPGA dispatch automatically.

### 6.8 Hardware requirements for local proving (development)

Per SP1 V6 docs:
- Minimum: 16GB RAM for compressed proofs of small programs
- Groth16 wrap: additional ~20 min on CPU (use network for Groth16 in dev)
- GPU (local): CUDA-capable GPU with CUDA toolkit installed; set `CUDA_VISIBLE_DEVICES`
- FPGA: requires custom integration — use prover network instead

For development and CI, use `Core` proofs. Switch to `Groth16` on the network for staging/production.

### 6.9 Cycle tracking and optimization

Use SP1's cycle tracker to measure proof cost during development:

```rust
// In program/src/main.rs (dev builds only)
sp1_zkvm::io::hint(&"start content hash");
// ... content hashing ...
sp1_zkvm::io::hint(&"end content hash");
```

Then in the script:
```rust
let (_, report) = client.execute(KAKO_ELF, &stdin).run().unwrap();
println!("cycles: {}", report.total_instruction_count());
println!("syscalls: {:?}", report.syscall_counts);
// Verify SHA_EXTEND and SHA_COMPRESS appear in syscall_counts
// If not: sha2 patch is not active — check Cargo.lock
```

Confirm precompiles are active:
```bash
cargo tree -p sha2
# Must show: sha2 v0.10.9 (https://github.com/sp1-patches/RustCrypto-hashes?tag=...)
# Not: sha2 v0.10.9 (registry+crates.io)
```

---

## 7. Multi-Chain Registry

### 7.1 Design principle

The Groth16 proof bytes produced by SP1 are chain-agnostic. A single proof generated once is verifiable on any chain that has an SP1 Groth16 verifier deployed. Kako deploys a thin registry contract/program on each target chain. The proof is submitted once to the prover network, then the proof bytes and public values are broadcast to whichever chains are configured.

### 7.2 Chain abstraction layer

```typescript
// registry/chain-adapter.ts

interface ChainAdapter {
  chain_id:       string;
  name:           string;
  anchor(cid: string, stamp: KakoStamp, proof_bytes: Uint8Array): Promise<string>; // tx hash
  lookup(cid: string): Promise<StampRecord | null>;
}

// Implementations:
// registry/adapters/evm.ts      → ethers.js / viem
// registry/adapters/solana.ts   → @solana/web3.js + sp1-solana program
// registry/adapters/sui.ts      → @mysten/sui.js
// registry/adapters/custom.ts   → pluggable for own chain (v0.3+)
```

On stamp, the add-in calls the Kako backend, which submits to all configured chains in parallel. The user selects a primary chain in the add-in settings. The KakoStamp embeds the CID; the verifier checks whichever chain the institution specifies.

### 7.3 EVM

SP1 Groth16 on-chain verification via Succinct's deployed `SP1VerifierGateway` contract:

```solidity
// contracts/src/KakoRegistry.sol

interface ISP1VerifierGateway {
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external;
}

contract KakoRegistry {
    ISP1VerifierGateway immutable SP1_GATEWAY;
    bytes32 immutable KAKO_VKEY;  // Set at deploy from `vk.bytes32()`

    struct StampRecord {
        bytes32 docHash;
        bytes32 sessionHash;
        bytes32 commitment;
        uint256 timestamp;
        bytes32 parentCid;
        address devicePubkey;  // Ed25519 pubkey encoded as address (truncated)
    }

    mapping(bytes32 => StampRecord) public stamps;

    function anchor(
        bytes32 cid,
        bytes calldata publicValues,
        bytes calldata proof
    ) external {
        SP1_GATEWAY.verifyProof(KAKO_VKEY, publicValues, proof);
        // Proof is valid. Decode and store.
        (PublicOutputs memory out) = abi.decode(publicValues, (PublicOutputs));
        stamps[cid] = StampRecord({
            docHash:    bytes32(out.doc_hash),
            sessionHash: bytes32(out.session_hash),
            commitment:  bytes32(out.document_commitment),
            timestamp:   out.seal_timestamp_ms / 1000,
            parentCid:   out.parent_cid,
            devicePubkey: address(bytes20(out.device_pubkey))
        });
        emit Anchored(cid, out.parent_cid, block.timestamp);
    }
}
```

SP1VerifierGateway deployed addresses: see `docs.succinct.xyz/docs/sp1/verification/contract-addresses`.

**Gas cost:** `SP1_GATEWAY.verifyProof` (Groth16) ≈ 270k gas. `KakoRegistry.anchor` total ≈ 290k gas. At 10 gwei on mainnet ≈ $0.06 per stamp. On L2s (Arbitrum, Base) ≈ $0.001.

### 7.4 Solana

Succinct provides `sp1-solana` crate. Groth16 verification via Solana's native BN254 precompiles. Cost: ~280K compute units.

```rust
// contracts/solana/src/lib.rs

use sp1_solana::{verify_proof, GROTH16_VK_BYTES};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshDeserialize, BorshSerialize)]
pub struct AnchorInstruction {
    pub cid:            [u8; 32],
    pub proof:          Vec<u8>,   // ~260 bytes Groth16
    pub public_values:  Vec<u8>,
}

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let ix = AnchorInstruction::try_from_slice(data)?;

    verify_proof(
        &ix.proof,
        &ix.public_values,
        &KAKO_VKEY_HASH,
        GROTH16_VK_BYTES,
    ).map_err(|_| ProgramError::InvalidArgument)?;

    // Write stamp record to Solana account
    let stamp_account = &accounts[0];
    // … serialize PublicOutputs into account data …

    Ok(())
}
```

Note: Solana's compute unit limit defaults to 200K. Requesting 300K in the transaction instruction is required. The `sp1-solana` library itself is not audited for production as of the current Succinct release — track audit status before production Solana launch.

### 7.5 Sui

Integration via Soundness Labs (collaboration with Succinct, Feb 2025). SP1 proofs verifiable in Sui Move modules. Implementation follows the same Groth16 pattern with Sui's BN254 precompile access. Track: `github.com/soundnesslabs` for the current verifier module address.

### 7.6 Custom chain (v0.3)

Any chain with BN254 elliptic curve support can verify SP1 Groth16 proofs. The verifier requires:
1. BN254 G1 and G2 point operations (pairing check)
2. The SP1 Groth16 verification key (`vk.bytes32()`)

SP1 will publish verifier implementations for additional VMs as the ecosystem grows. For Kako's own chain (v0.3), deploy the Solidity verifier to an EVM-compatible custom chain, or implement a native verifier for the target VM.

---

## 8. KakoStamp Structure

```typescript
// protocol/stamp.ts

export interface KakoStamp {
  protocol_version:     "kako/0.1";
  cid:                  string;    // keccak256(proof_bytes ‖ public_values), hex
  doc_hash:             string;    // SHA-256 of canonicalized content, hex
  session_hash:         string;    // SHA-256 of canonical events JSON, hex
  document_commitment:  string;    // compound SHA-256 commitment, hex
  proof_bytes_b64:      string;    // Groth16 proof, base64 (~260 bytes → ~360 chars)
  public_values_b64:    string;    // PublicOutputs encoded, base64
  device_pubkey:        string;    // Ed25519 pubkey, hex
  seal_timestamp_iso:   string;    // ISO 8601
  paste_event_count:    number;
  ai_insertion_count:   number;
  total_typed_chars:    number;
  parent_cid:           string | null;
  chain_anchors: {
    chain:   string;               // "ethereum", "solana", "sui", etc.
    tx_hash: string;
    block:   number;
  }[];
}
```

Stored at `docProps/custom.xml` under namespace `http://kako.io/v1/stamp`.

---

## 9. Verification API

```
POST /api/verify
Content-Type: multipart/form-data
Body: file=<docx binary>

Steps:
1. Parse .docx ZIP → extract customXml/item*.xml → find kako namespace
2. Deserialize KakoStamp
3. Extract document body → run canonicalization pipeline (Rust, same impl as circuit)
4. doc_hash_current = SHA-256(canonical content)
5. Assert doc_hash_current === stamp.doc_hash                → INTEGRITY_FAIL
6. Deserialize proof_bytes_b64 and public_values_b64
7. Verify Groth16 proof locally using SP1 verifier binary     → PROOF_INVALID
8. Lookup stamp.cid on configured chain(s)                    → NOT_ANCHORED
9. Return ProvenanceReport
```

```typescript
// verifier-api/src/routes/verify.ts

interface ProvenanceReport {
  status:             "VERIFIED" | "CONTENT_MODIFIED" | "PROOF_INVALID" | "NOT_ANCHORED";
  cid:                string;
  integrity_ok:       boolean;   // SHA-256 recompute matches stamp
  proof_valid:        boolean;   // Groth16 locally verified
  chain_anchors:      ChainAnchor[];

  session: {
    typed_chars:       number;
    pasted_chars:      number;
    ai_inserted_chars: number;
    paste_count:       number;
    ai_count:          number;
    duration_minutes:  number;
    avg_wpm:           number;
  };

  provenance_chain:    KakoStamp[];  // full chain from genesis
  paragraph_timeline:  ParagraphSnapshot[];
  seal_timestamp_iso:  string;
  device_pubkey:       string;
}
```

---

## 10. Repository Structure

```
kako/
├── packages/
│   │
│   ├── word-addin/                      TypeScript · Word JS API
│   │   ├── src/
│   │   │   ├── evidence/
│   │   │   │   ├── collector.ts         hooks onContentChanged, BeforePaste, Copilot
│   │   │   │   ├── classifier.ts        ★ TYPED/PASTE/AI_INSERTION rules
│   │   │   │   └── session.ts           bundle accumulation, offline queue
│   │   │   ├── canonicalize/
│   │   │   │   ├── pipeline.ts          ★ 6-stage pipeline, must match Rust impl
│   │   │   │   └── schema.ts            EventRecord, ParagraphSnapshot types
│   │   │   ├── stamp/
│   │   │   │   ├── save-interceptor.ts  ★ DocumentBeforeSave hook, blocks save
│   │   │   │   ├── embedder.ts          write/read CustomXmlPart (kako namespace)
│   │   │   │   ├── extractor.ts         tamper detection on open
│   │   │   │   └── pdf-embedder.ts      XMP metadata via backend
│   │   │   └── taskpane/
│   │   │       ├── taskpane.html
│   │   │       └── taskpane.ts          status display, session stats, retry
│   │   └── manifest.xml
│   │
│   ├── sp1-prover/                      Rust · SP1 V6 Hypercube
│   │   ├── lib/
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── types.rs             ★ EvidenceBundle, PublicOutputs (build first)
│   │   │       └── canon.rs             canonical_events_json() — must match TS pipeline
│   │   ├── program/
│   │   │   ├── src/main.rs              ★ ZK circuit (SHA-256 + Ed25519 + commit)
│   │   │   └── Cargo.toml              sha2 + curve25519-dalek sp1-patches applied
│   │   └── script/
│   │       └── src/bin/
│   │           ├── prove.rs             generate proof (core/groth16/plonk)
│   │           ├── verify.rs            verify locally with vk
│   │           └── cycle-check.rs       print cycle report + syscall_counts
│   │
│   ├── verifier-api/                    TypeScript · Bun
│   │   └── src/
│   │       ├── routes/
│   │       │   ├── verify.ts            POST /api/verify
│   │       │   └── report.ts            GET /api/report/:cid
│   │       └── services/
│   │           ├── canon.ts             Rust canon binary invoked via child_process
│   │           ├── sp1-verify.ts        Groth16 local verify via SP1 binary
│   │           └── registry.ts          multi-chain CID lookup
│   │
│   └── verifier-web/                    Next.js
│       └── src/
│           ├── pages/
│           │   ├── index.tsx            upload + call /api/verify
│           │   └── report/[cid].tsx     provenance report (public URL)
│           └── components/
│               ├── ProvenanceReport.tsx
│               └── SessionTimeline.tsx
│
├── contracts/
│   ├── evm/
│   │   ├── src/KakoRegistry.sol         calls SP1VerifierGateway, stores StampRecord
│   │   └── foundry.toml
│   └── solana/
│       └── src/lib.rs                   sp1-solana Groth16 verify + account storage
│
└── docs/
    ├── CANONICALIZATION.md
    ├── CIRCUIT.md
    └── MULTICHAIN.md
```

---

## 11. Build Order

Strict dependency order. Each step requires the previous to be complete and tested.

```
Step 1:  sp1-prover/lib — types.rs, canon.rs
         Defines EvidenceBundle and PublicOutputs.
         All other packages depend on these types.

Step 2:  sp1-prover/program — main.rs
         Write circuit. Run with `cargo prove --release` locally using Core proofs.
         Capture vk (verifying key) — every downstream component needs it.
         Confirm sha2 + curve25519-dalek patches active via cycle-check.rs.

Step 3:  word-addin/canonicalize — pipeline.ts, schema.ts
         Pure TypeScript. No Word dependency.
         Unit test exhaustively against the Rust canon.rs output.
         Canonicalization divergence between TS and Rust = broken verification.

Step 4:  word-addin/evidence — collector.ts, classifier.ts, session.ts
         Requires Word desktop sideloading for testing.
         Test all event types including Copilot AI_INSERTION surface.

Step 5:  word-addin/stamp — save-interceptor.ts, embedder.ts, extractor.ts
         Test full round-trip: open → edit → save intercepted → proof generated
         → stamp embedded → document saved → reopen → hash verified.

Step 6:  verifier-api
         Depends on vk from Step 2. Implement Groth16 local verify first
         before wiring chain lookups.

Step 7:  contracts (EVM first, then Solana)
         Deploy KakoRegistry to Sepolia. Get contract address.
         Test anchor() with a proof from Step 2.

Step 8:  verifier-web
         Wire to verifier-api. Test full upload → report flow.
```

---

## 12. MVP Scope (v0.1)

**In scope:**
- Save interception with proof generation (DocumentBeforeSave hook)
- Evidence collection: TYPED, PASTE_CLIPBOARD, AI_INSERTION events only
- Canonicalization pipeline (TS + matching Rust impl)
- SP1 circuit: SHA-256 doc hash + session hash + Ed25519 device sig
- Compressed proof for local verification (no on-chain in v0.1)
- KakoStamp in CustomXmlPart (no footer, no body ContentControl)
- Tamper detection on document open
- Single-page verifier: upload .docx → extract stamp → verify locally → show report
- Local prover only (no Succinct network in v0.1)

**Out of scope (v0.2+):**
- On-chain anchoring (EVM, Solana, Sui)
- Groth16 proof type
- Succinct prover network integration
- PDF XMP export
- Parent CID chain
- Multi-author sessions
- Browser extension
- Credential attestation

**v0.1 acceptance criterion:**
Verifier at `kako.io/verify` receives a `.docx` file, extracts the KakoStamp, re-canonicalizes the document, verifies the compressed SP1 proof locally, and renders a provenance report showing session duration, paste count, AI insertion count, and proof validity — all within 10 seconds.
