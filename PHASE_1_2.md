# Phase 1 & 2 Engineering Notes

## Completed

- Accepted TCP connections are handled in independent Tokio tasks.
- Noise handshakes have a 10-second timeout on both initiator and responder.
- Callback pairing confirmation expires after 60 seconds and clears only the matching pending request.
- Dial markers are removed after every connection attempt, including normal session termination, so trusted peers can reconnect.
- Established sessions send PING heartbeats every 15 seconds and close after 45 seconds without a PONG.
- Incoming connections briefly wait for mDNS metadata after Noise authentication, fixing the event-order race without treating discovery metadata as identity or trust.
- mDNS peers advertising a protocol version other than `1` are ignored.
- Protocol constants are centralized in `core/src/protocol.rs`; they are not settings.

## Cleanup decisions

- The unused `protocol` crate and its workspace dependency were removed. Live packet/session code is in `core`.
- The uncompiled `daemon/src/features/battery.rs` was removed. Battery remains a future feature.
- `NotPaired` was removed from the unused UniFFI error surface; no current API operation needed it.
- The stale `SPEC.md` reference now points to `md files/scope v1.md`.
- Ed25519 remains in the persisted identity because the current protocol specification still defines it. The runtime does not yet use it for signing; adding a signing exchange is intentionally deferred rather than invented here.

## Tests

Added focused tests for protocol-version validation, pairing and connection-marker cleanup identity checks, PING/PONG routing, and decoding unknown packet types.

## Remaining limitations

- The protocol specification still describes future HELLO/version re-confirmation, while the live implementation validates the mDNS version only.
- Session acceptance, pairing, and discovery still use the existing global in-process state and have no multi-request pairing queue.
- The Android native libraries are checked-in debug builds and must be regenerated whenever the Rust/UniFFI contract changes.
