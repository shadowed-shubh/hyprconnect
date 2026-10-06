# HyprConnect Changes Report

This document describes the current working-tree changes for Claude or another developer continuing the implementation.

## Executive summary

The daemon was moved from a mostly single-stream PING prototype toward a long-lived encrypted session that can support multiple independent features. PING/PONG and battery handling are now separated into feature modules. Battery updates can be pushed asynchronously to every active session. The session now has one reader and one writer task, with an outbound channel used for replies and locally generated events.

The project currently compiles and tests successfully with:

```text
cargo check
cargo test
```

There is still one warning because the Ed25519 verifying key is stored but not yet used.

## UniFFI foundation milestone

The new `core` crate is now configured as the shared UniFFI library without exposing feature implementations yet.

### Added `core/src/hyprconnect.udl`

The UDL currently contains only the `hyprconnect` namespace. This intentionally leaves the foreign-language API surface empty while the bridge architecture is being established. Feature APIs can be added later without mixing their implementation into the initial UniFFI setup.

### Updated `core/src/lib.rs`

The crate now includes generated UDL scaffolding with:

```rust
uniffi::include_scaffolding!("hyprconnect");
```

The previous `setup_scaffolding!()` and experimental callback proc-macro declarations were removed. A crate must use either proc-macro scaffolding or UDL scaffolding, not both.

### Rust edition compatibility

`core/Cargo.toml` uses Rust edition 2021 because the current UniFFI 0.28 generated scaffolding emits `#[no_mangle]`, which Rust 2024 treats as an unsafe attribute. The daemon and protocol editions remain unchanged.

### Feature separation

No battery or other feature implementation was moved into the UniFFI API yet. `core/src/features/` currently remains limited to the existing ping module, while feature-specific APIs will be added deliberately in a later step.

## Files added

### `daemon/src/features/mod.rs`

Declares the feature modules:

```rust
pub mod battery;
pub mod ping;
```

This establishes the intended structure for adding future features such as clipboard, notifications, and file transfer.

### `daemon/src/features/ping.rs`

Owns PING/PONG behavior:

- Defines `PING` and `PONG` packet type constants.
- Provides `ping()` and `pong()` packet constructors.
- Provides `is_ping()` and `is_pong()` helpers.

The session no longer constructs PING/PONG packets directly with raw JSON.

### `daemon/src/features/battery.rs`

Owns battery functionality:

- Reads battery percentage and state from Linux UPower over system D-Bus.
- Converts UPower state values into a `charging` boolean.
- Builds `BATTERY_REQUEST` packets.
- Builds `BATTERY_RESPONSE` packets.
- Recognizes `BATTERY_REQUEST`, `BATTERY_RESPONSE`, and `BATTERY_CHANGED` packets.
- Logs received battery data.
- Watches UPower property changes and broadcasts battery updates.

Battery read failures return a valid response with nullable fields instead of terminating the session:

```json
{
  "level": null,
  "charging": null,
  "error": "no_battery"
}
```

## Session architecture changes

### `daemon/src/session.rs`

The session was substantially changed.

#### Previous behavior

The session directly wrote packets to the same stream from the read loop. This worked for the initial PING prototype but could not safely support asynchronous battery events. The previous code also contained placeholders for PONG and battery responses because the writer half had been moved into another task.

#### Current behavior

After pairing, the TCP stream is split into:

- a read half used by the incoming packet loop;
- a write half owned by a dedicated writer task.

The session creates:

```text
mpsc channel       outbound replies and requests
broadcast receiver battery change events
Mutex<TransportState> shared Noise transport state
```

The writer task selects between:

1. Packets received through the outbound channel.
2. Battery updates received from the global battery broadcast channel.

All encrypted writes are serialized through the shared transport mutex. This prevents concurrent writes from corrupting Noise transport state or interleaving frames on the TCP stream.

#### Initial packets

After pairing, the session queues:

```text
PING
BATTERY_REQUEST
```

#### Incoming packet routing

| Packet | Behavior |
|---|---|
| `PING` | Queue a `PONG` reply |
| `PONG` | Log successful round trip |
| `BATTERY_REQUEST` | Read local battery and queue `BATTERY_RESPONSE` |
| `BATTERY_RESPONSE` | Log peer battery state |
| `BATTERY_CHANGED` | Log peer battery update |
| Unknown type | Log and ignore it |

Unknown packets are intentionally non-fatal so future protocol versions can add packet types without breaking older peers.

## Battery watcher integration

### `daemon/src/main.rs`

The daemon now creates a broadcast channel:

```rust
let (battery_tx, _) = broadcast::channel::<Packet>(32);
```

It starts the UPower watcher in a background task. The sender is passed to discovery and used for incoming sessions.

Each active session receives its own subscription using:

```rust
battery_tx.subscribe()
```

This means one battery event can be delivered to all connected peers without the battery module needing to know about individual TCP sessions.

## Discovery changes

### `daemon/src/discovery.rs`

Discovery now accepts the global battery broadcast sender and subscribes each outgoing session to it.

The outgoing connection path now passes the battery receiver into `run_session`.

The existing deterministic dial tie-break remains in place: the device with the smaller device ID dials, while the other device waits for the incoming connection.

### Retry bug fixed

Previously, a discovered device was added to `already_connected` before the connection and handshake succeeded. If TCP connection or Noise handshake failed, the device stayed marked as connected forever and could not be retried.

The failed connection task now removes the device ID from the set, allowing a later mDNS announcement to retry the connection.

## Incoming connection changes

`daemon/src/main.rs` now passes the accepted `TcpStream` by value into `run_session`, together with a battery subscription.

This fixes the previous compile error where `&mut TcpStream` was passed even though `run_session` needed to own the stream and split it into read/write halves.

## Packet and Noise framing changes

### `daemon/src/packet.rs`

`send_packet()` and `recv_packet()` now accept generic async readers and writers:

```rust
W: AsyncWrite + Unpin
R: AsyncRead + Unpin
```

This allows them to work with `TcpStream::into_split()` halves.

The unused direct `TcpStream` import was removed.

### `daemon/src/noise.rs`

The frame read/write helpers were also generalized to accept any async reader or writer. Encrypted packet transport can therefore operate over split TCP halves.

The existing Noise configuration remains:

```text
Noise_XX_25519_ChaChaPoly_BLAKE2s
```

The transport still uses a two-byte big-endian length prefix around each encrypted Noise message.

## TLS removal

### `daemon/src/tls.rs`

The old TLS implementation was deleted. The daemon now relies on Noise XX for:

- key exchange;
- encrypted transport;
- authenticated static X25519 keys.

The old TLS dependencies remain visible in the dependency file and should be reviewed for removal if they are no longer referenced anywhere else.

## Dependency changes

### `daemon/Cargo.toml`

Added Tokio synchronization support:

```toml
"sync"
```

Added:

```toml
zbus = "4"
futures-util = "0.3"
```

These support UPower D-Bus access and asynchronous property-change streams.

`Cargo.lock` was updated accordingly.

## Errors fixed

The original code did not compile because both session call sites used the old function shape:

```text
run_session(&mut stream, transport, my_pub, remote_pub, remote_name)
```

The function now requires an owned stream and a battery receiver:

```text
run_session(stream, transport, my_pub, remote_pub, remote_name, battery_rx)
```

The original session also had non-functional placeholders for replies:

- PING was received but no PONG was sent.
- BATTERY_REQUEST was received but no BATTERY_RESPONSE was sent.

Both now send real packets through the writer channel.

## Verification

The following commands currently pass:

```sh
cargo fmt --all
cargo check
cargo test
```

The UniFFI scaffolding is generated during the normal Cargo build from `core/src/hyprconnect.udl`.

Test results include:

- protocol identity parsing test: passed;
- protocol packet round-trip test: passed;
- daemon unit-test target: passed with no tests defined;
- protocol documentation tests: passed.

## Remaining warnings and limitations

### Unused Ed25519 verifying key

`daemon/src/identity.rs` stores `ed25519_verifying`, but no code currently reads it. This produces a compiler warning. It should remain until the protocol adds explicit Ed25519 signing or peer identity authentication. It can then be used for signatures, or removed if X25519-only identity is chosen deliberately.

### No heartbeat or automatic session reconnect yet

PING/PONG currently proves a round trip during session startup and responds to peer PING packets. It is not yet a periodic heartbeat system. A future implementation should add:

- periodic PING timer;
- PONG timeout;
- session failure detection;
- reconnect/backoff policy;
- removal of established connections from the active set when they close.

### Incoming connections are handled sequentially

The current accept loop performs the handshake and then awaits the entire session before accepting another incoming connection. If multiple incoming peers must be supported, the accepted connection should be moved into a spawned task immediately.

### Pairing input is blocking

`confirm_via_stdin()` uses synchronous `stdin.read_line()` inside an async session path. This is acceptable for the current CLI prototype but should eventually be moved to a blocking task or replaced by a desktop/Android approval UI.

### Battery event de-duplication

The comments describe broadcasting only when the battery value actually changes. The current watcher broadcasts whenever UPower emits a percentage or state signal. It should compare the new values with the previous values before broadcasting if duplicate events need to be suppressed.

### UPower dependency and environment

Battery support requires:

- Linux;
- a running system D-Bus;
- UPower;
- the `DisplayDevice` UPower object.

If those are unavailable, request handling reports `no_battery`, while the long-running watcher logs an error and stops.

## Suggested next work for Claude

1. Spawn each accepted connection so multiple incoming peers can stay connected.
2. Track established sessions separately from dialing attempts.
3. Add periodic heartbeat and reconnect logic.
4. Add tests for ping routing, battery packet parsing, and unknown packet handling.
5. Remove unused TLS dependencies after confirming no code references them.
6. Decide whether Ed25519 will be used for signatures or removed from the active identity structure.
7. Add feature modules following the same pattern as `ping.rs` and `battery.rs`.


## UniFFI pairing and device-registry wiring

The UniFFI-facing API is now connected to the existing core logic.

### Pairing confirmation API

The UDL now exposes:

```udl
void confirm_pairing(string device_id, boolean accepted);
```

Pairing uses a single global pending-pairing slot containing the device ID and a Tokio oneshot sender. `pair(device_id)` rejects a second simultaneous request with `PairingInProgress`. The callback receives the fingerprint code, and `confirm_pairing()` completes the waiting future only when the device ID matches. Empty, stale, or mismatched confirmations are ignored.

The existing CLI path remains available through `StdinConfirmer`; the shared session code now accepts a `PairingConfirmer` abstraction instead of calling stdin directly.

### Discovery registry

`core/src/discovery.rs` now retains mDNS devices in a process-wide synchronized map and exposes:

- `discovered_devices()`
- `get_discovered(device_id)`
- `get_discovered_by_address(address)`

The registry retains device ID, name, type, IPv4 address, and port. Devices are recorded before the deterministic dial tie-break, so a device that waits for the peer to dial is still visible to `list_devices()`.

### Trust schema

`TrustedDevice` now stores:

- `device_id`
- `device_name`
- `device_type`
- `x25519_pub_hex`

`add_trusted()` and all session call sites were updated to write the complete metadata. The old local development trust store was removed before validation; the second requested `/tmp` trust-store path was absent.

### Device listing

`list_devices()` merges trusted devices loaded from the trust store with currently discovered devices. Trusted records take precedence and are returned with `trusted: true`; discovered-only records are returned with `trusted: false`.

### Validation

The completed wiring passes:

```sh
cargo fmt --all
cargo check --workspace
cargo test --workspace
git diff --check
```

### Incoming metadata safety

Incoming connections no longer enter pairing with a fabricated `unknown` device ID. If the peer's address is not present in the discovery registry after the Noise handshake, the daemon logs the condition and closes the connection. This prevents multiple unresolved peers from overwriting the same trust-store entry.


## Discovery list-only mode

Discovery identity now carries an `auto_pair` flag:

- `true`: preserve the CLI daemon's existing automatic dial and CLI pairing behavior.
- `false`: advertise, browse, and populate the discovered-device registry without dialing or pairing.

This keeps Android discovery passive so the UI can display nearby devices and invoke `pair(device_id)` only after the user explicitly selects one.

The UDL also exposes asynchronous `start_discovery()`. It starts the core mDNS advertiser/browser and the incoming TCP accept loop as background tasks using callback-based pairing and list-only mode. The CLI daemon now reuses the same core startup path with `auto_pair: true` and `StdinConfirmer`.

The `DeviceEventCallback::on_pairing_code` callback now receives both `device_id` and the fingerprint `code`, allowing Android to call `confirm_pairing(device_id, accepted)` for the correct pending request.


## Android build verification

The Android project was built with:

```sh
gradle --no-daemon --console=plain -p android assembleDebug
```

The build now succeeds and produces:

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

Android-side fixes made during validation:

- Replaced the unavailable `org.mozilla.uniffi:uniffi:0.28.0` dependency with the JNA runtime required by UniFFI Kotlin bindings.
- Fixed recursive Kotlin wrapper calls by aliasing generated UniFFI functions.
- Removed a duplicate generated Kotlin binding source.
- Fixed missing `DeviceInfo` imports.
- Fixed coroutine `cancel()` imports.
- Replaced the Compose ViewModel delegate with the Android `viewModels()` delegate.
- Replaced missing launcher resource references with an available platform icon.

The build emits environment warnings because the machine uses Java 26 with older Kotlin/Android tooling and has an Android SDK XML version mismatch. The current checked-in native library contains only the `arm64-v8a` ABI, so an x86/x86_64 emulator will need an additional native library build.
