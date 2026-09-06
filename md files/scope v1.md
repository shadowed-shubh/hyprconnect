# HyprConnect Protocol Specification v0.1

Status: Draft. Governs the Rust `protocol` crate, shared by the Linux
daemon and the Android app (via UniFFI). Explicitly NOT compatible
with KDE Connect's wire protocol.

## 1. Scope

v0.1 covers: identity, discovery, pairing/trust, session establishment,
message-type packets (ping, battery, clipboard, notification), and one
resource type (file transfer). Explicitly deferred to v0.2+: streams
(audio/video/screen/remote input), context/Handoff, QUIC, BLE discovery,
multi-device fan-out beyond one phone + one desktop.

## 2. Architecture

    UNKNOWN -> DISCOVERED -> PAIRING -> TRUSTED -> CONNECTED -> ONLINE
                                            |
                                        REVOKED (user removes trust)

A device is DISCOVERED the moment we see it on the network. Discovery
never implies trust. TRUSTED means we've completed pairing and stored
the peer's public key. CONNECTED/ONLINE distinguish "session open" from
"session open and identity re-verified this run."

## 3. Device Identity

Each device generates once, on first run, and persists to disk:
- An Ed25519 keypair (identity signing key)
- An X25519 keypair, derived for use in the Noise handshake (via `snow`,
  which handles this internally given a static keypair)
- A `device_id`: UUID v4, no format constraints beyond that (no 32-38
  char / alphanumeric requirement — that was a KDE Connect cert-CN
  legacy constraint we don't inherit)
- `device_name`, `device_type` (string enum: "desktop" | "phone")

The private key NEVER leaves the device, never appears in any packet,
never crosses the UniFFI boundary to Kotlin in raw form — Kotlin only
ever calls Rust functions that use the key internally.

## 4. Discovery

mDNS/DNS-SD, service type `_hyprconnect._tcp.local`. TXT record
advertises: `device_id`, `device_name`, `device_type`, `port`,
`protocol_version`. No capability list in the discovery record —
capabilities are exchanged post-pairing (see §12), keeping discovery
payloads small.

## 5. Pairing & Trust

1. Initiator (whichever side the user starts pairing from) opens a
   connection and performs a Noise_XX handshake (see §7) — this pattern
   exchanges public keys as PART of the handshake itself, so no
   separate "send me your identity" step is needed pre-handshake.
2. Both sides independently compute a fingerprint:
   `SHA256(initiator_pubkey || responder_pubkey)`, truncated to 6 bytes,
   rendered as a 6-digit decimal code (easier for a human to read aloud
   / compare than hex).
3. Both UIs display this code. Human confirms match on both devices
   (or: initiator confirms, responder gets a simple accept/reject —
   implementation-defined, not protocol-defined; see §19).
4. On mutual accept, each side stores the other's public key + device_id
   + device_name, tagged trusted.
5. `PAIR_REJECT {reason: string}` on decline. Connection closes.

## 6. Trust Storage

Local JSON file (path implementation-defined — for the Linux daemon,
`~/.config/hyprconnect/trusted_devices.json`), one entry per peer:
`{device_id, device_name, public_key_hex, paired_at}`. No expiry by
default; revocation is a local delete-entry action, not a protocol
message (the other side simply fails re-authentication next connect
and must be re-paired).

## 7. Transport & Security

TCP, port TBD (avoid colliding with real KDE Connect's 1716 if both
might run on the same machine during development — suggest 17160).
Noise Protocol Framework, pattern **Noise_XX** (`snow` crate), running
directly over the TCP stream — no TLS, no X.509, no certificates.
Noise_XX is chosen specifically because:
- Both static keys are exchanged and authenticated as part of the
  handshake itself (no separate identity packet needed).
- No client/server role asymmetry to get wrong — unlike TLS, initiator
  and responder are symmetric in complexity, sidestepping the entire
  category of bug that cost hours during KDE Connect protocol work.
- It's the same primitive WireGuard and Syncthing use for this exact
  "long-term keypair, pair once, trust forever" pattern.

After the handshake completes, all further bytes on the connection are
Noise transport messages (encrypted+authenticated), framed as below.

## 8. Framing

Newline-delimited JSON, one packet per line — same framing style
validated in the KDE Connect-compatible prototype. Binary wire format
is explicitly deferred; JSON's debuggability (readable in raw `nc`/log
output) outweighs the overhead at v0.1 message volumes.

    {"type": "<packet_type>", "req_id": <u32|null>, "body": {...}}

`req_id` is present and non-null only on request/response pairs (see
§10). Events and one-way commands omit it (`null`).

## 9. Packet Types (v0.1)

Session: `HELLO`, `PING`, `PONG`, `GOODBYE`
Pairing: `PAIR_REQUEST`, `PAIR_ACCEPT`, `PAIR_REJECT`
Device: `CAPABILITIES`, `BATTERY_REQUEST`, `BATTERY_RESPONSE`, `BATTERY_CHANGED`
Clipboard: `CLIPBOARD_CHANGED`
Notification: `NOTIFICATION_EVENT`, `NOTIFICATION_ACTION`
File: `FILE_OFFER`, `FILE_ACCEPT`, `FILE_REJECT`, `FILE_COMPLETE`, `FILE_CANCEL`
Error: `ERROR`

## 10. Request/Response/Event Semantics

- **Request**: has `req_id`, expects exactly one `*_RESPONSE` with the
  same `req_id` back. E.g. `BATTERY_REQUEST` -> `BATTERY_RESPONSE`.
- **Event**: `req_id: null`, fire-and-forget, no response expected.
  E.g. `BATTERY_CHANGED`, `CLIPBOARD_CHANGED`.
- `req_id` is a `u32`, generated by whichever side sends the request,
  incrementing per-connection (resets each new session — no need for
  it to be globally unique, only unique within one open connection).
- No built-in retry/ack layer in v0.1 — TCP already guarantees delivery
  order within a connection; if the connection drops, the whole session
  is considered dead and must be re-established, not resumed
  packet-by-packet. Idempotency/replay protection deferred until a
  concrete feature needs it (file resume is the one exception, see §13).

## 11. Errors

    {"type": "ERROR", "req_id": <matching request's id | null>,
     "body": {"code": "<UNSUPPORTED_FEATURE|INVALID_PAYLOAD|...>",
               "message": "<human-readable>"}}

Unknown packet types are logged and ignored, not treated as fatal —
this is the actual versioning mechanism (see §14): a v0.1 peer talking
to a hypothetical v0.2 peer simply ignores packet types it doesn't
recognize rather than erroring.

## 12. Capabilities

Sent once, right after pairing completes (or on reconnect):

    {"type": "CAPABILITIES", "body": {
        "capabilities": [
            {"name": "clipboard", "version": 1},
            {"name": "battery", "version": 1},
            {"name": "notifications", "version": 1},
            {"name": "file_transfer", "version": 1}
        ]
    }}

`Vec<{name, version}>` rather than bitflags — allows a device to
support "clipboard v1" while a future device supports "clipboard v2",
without needing a protocol-wide version bump for one feature to evolve.

## 13. File Transfer

    FILE_OFFER {transfer_id: uuid, filename, size_bytes, sha256}
      -> FILE_ACCEPT {transfer_id} | FILE_REJECT {transfer_id, reason}
      -> raw bytes on a SECOND Noise-secured connection, new handshake,
         same paired identity
      -> FILE_COMPLETE {transfer_id, sha256_actual}

Resume: if a partial file with matching `transfer_id` exists locally,
receiver sends `FILE_ACCEPT {transfer_id, resume_offset}` instead of a
bare accept; sender seeks to that offset before streaming. Integrity
verified by comparing `sha256_actual` against the original offer's hash
after transfer completes. Concurrent transfers: each gets its own
second connection; no cross-transfer scheduling logic in v0.1.

## 14. Versioning

`protocol_version: u32` sent in the mDNS TXT record and re-confirmed
in `HELLO`. A version mismatch is logged, not fatal, unless the major
version differs meaningfully enough to break framing itself (v0.1 has
only one version, so this is aspirational for now). Unknown packet
types and unknown JSON fields within a known packet type are both
ignored rather than rejected — this is what lets v0.1 and a future
v0.2 peer interoperate on the packet types they share in common.

## 15. Clipboard Sync Loop Prevention

    {"type": "CLIPBOARD_CHANGED", "body": {
        "origin_device_id": "<uuid>",
        "content_type": "text/plain",
        "content": "...",
        "timestamp": <unix_ms>
    }}

Each side tracks the last clipboard content IT applied via a remote
sync (not user-initiated). When the local clipboard-watcher fires
immediately after applying a remote update, and the new content
matches what was just remotely applied, that local-change event is
suppressed instead of re-broadcast. This is the actual loop-breaker —
comparing against "what did I just apply," not comparing device IDs.

## 16. UniFFI Boundary (Rust <-> Kotlin)

Rust owns: the entire connection/session loop, Noise handshake state,
private key storage and use, all packet encode/decode. Exposed to
Kotlin: high-level async functions (`pair(device_id)`,
`send_clipboard(text)`, `list_devices()`) and a callback trait
(`on_notification(...)`, `on_battery_changed(...)`,
`on_pairing_code(code: String)`) that Kotlin implements and Rust
invokes. Kotlin never sees a raw socket, a Noise session object, or key
bytes. The Android foreground service's only job is: start the Rust
core on service start, forward its callbacks into Android
notification/clipboard APIs, stop it cleanly on service stop.

## 17. Security Considerations

- Fingerprint comparison (§5) is the only MITM defense — same
  trust-on-first-use model as SSH/Signal safety numbers. No CA, by
  design, since there's no third party to trust.
- Trust storage (§6) is unencrypted local JSON. Anyone with local
  filesystem access to a paired device can read peer public keys
  (not private keys) and device metadata. Acceptable for v0.1 (matches
  KDE Connect's own trust storage security level); revisit if this
  ever needs to defend against a compromised local user.
- No forward secrecy beyond what `Noise_XX` provides per-session
  (rekeying is out of scope for v0.1 — sessions are short-lived,
  re-established per connection).

## 18. Extensibility (deferred, not designed)

Streams (audio/video/screen/remote input), Handoff/context, QUIC
transport, BLE discovery: intentionally undesigned in v0.1. The
`{type, body}` framing and unknown-type-ignored versioning rule (§14)
are the seams that should make adding these later possible without
breaking v0.1 peers — but the actual design work happens when there's
a working v0.1 to build on top of, not before.
