# HyprConnect

Device continuity between an Android phone and a Linux/Hyprland desktop.
HyprConnect is inspired by KDE Connect and Apple Continuity, but built on
our **own protocol** (not KDE Connect compatible).

## Architecture Diagram

Interactive runtime architecture diagram (hosted on GitHub Pages):

**[View Architecture Diagram →](https://shadowed-shubh.github.io/hyprconnect/)**

The diagram shows:
- Platform split: Android Device (UniFFI + Kotlin) ↔ Linux Desktop (native Rust) via shared Core Library
- Network path: mDNS discovery → TCP transport → Noise_XX handshake → Encrypted session → Typed feature packets
- Trust model: Ed25519/X25519 device identity, trust-on-first-use pairing with 6-digit fingerprint verification
- Security boundaries: Encrypted Channel (Noise_XX), Trust Boundary (TOFU)

## What it is

HyprConnect wirelessly links a phone and a desktop so they behave like one
device:

- **Implemented foundation** — discovery, Noise_XX sessions, TOFU pairing,
  trust storage, device listing, and PING/PONG liveness
- **Planned features** — clipboard, notifications, battery, and file transfer
- *(planned)* streams: audio/video/screen/remote input

The stack is designed around a simple, debuggable, own-protocol design:

- **Noise Protocol Framework** (`Noise_XX`) for encryption — long-term
  Ed25519/X25519 keypairs, pair once, trust forever (the model WireGuard
  and Syncthing use). X25519 is the static key used by the current Noise
  handshake; Ed25519 remains part of the persisted device identity for the
  protocol's identity/signing architecture. No TLS or X.509 certificates.
- **mDNS/DNS-SD discovery** (`_hyprconnect._tcp.local`)
- **Secure pairing** with a human-confirmed fingerprint code
  (trust-on-first-use, like SSH)
- **Length-prefixed encrypted JSON** framing inside Noise transport messages

## Repo layout

```
hyprconnect/
├── core/              # shared core crate — discovery, noise, session, trust, pairing
├── md files/
│   └── scope v1.md    # the protocol spec, the source of truth
├── daemon/            # thin Linux daemon/CLI
└── android/           # Android app (UniFFI bindings + Kotlin UI)
```

The `core` crate is shared by both platforms. On Android it's reused
as-is via **UniFFI** bindings (no Kotlin reimplementation of the wire
protocol).

### Identity and trust

Each installation stores a persistent `identity.json` containing the device
ID, Ed25519 identity key, and X25519 Noise key. The current pairing fingerprint
and trust store use the authenticated X25519 public key. Ed25519 signing or a
separate identity-signature exchange is intentionally reserved for a future
protocol revision; retaining the key now keeps the implementation aligned with
the protocol specification without inventing an unused wire message.

New identity files are written with owner-only permissions (`0600` on Unix).

## Status

**In progress — v0.1, early.** The daemon and Android client share the Rust
core. mDNS discovery, the Noise_XX handshake, pairing, trust storage,
encrypted sessions, and PING/PONG are implemented. Clipboard, battery,
notifications, and file transfer remain planned.

A KDE Connect-compatible prototype (discovery → TLS → pairing → ping)
was previously built and validated against a real Android phone; it proved
out the architecture and informed the switch to our own, simpler protocol.

## Prerequisites

- Rust stable with the workspace targets installed for Linux and Android
- JDK 17
- Android SDK platform 34 and build-tools 34.0.0
- Android NDK 30.0.16248370

Set `ANDROID_SDK_ROOT` when the SDK is not at `~/Android/Sdk`. The Android
script also accepts `ANDROID_NDK_HOME` or `HYPRCONNECT_NDK_VERSION`.

## Build & run

Linux daemon:

```sh
cargo run -p daemon
```

On first run it generates a persistent device identity at
`~/.config/hyprconnect/identity.json`.

Android application and native core:

```sh
./scripts/build-android.sh
```

This is the canonical Android workflow. It builds the Rust core for
`x86_64-linux-android` and `aarch64-linux-android`, regenerates both tracked
UniFFI Kotlin binding copies, updates the matching checked-in native
libraries, and runs the Gradle debug build. The script intentionally keeps the
`.so` files tracked because the current Android project consumes `jniLibs`
directly and has no Gradle/cargo native build integration.

Rust checks:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

GitHub Actions runs these checks and then calls the same Android build script.

## Versioning and releases

The current versions are protocol `1`, Rust core/daemon `0.1.0`, and Android
application `0.1.0`. Protocol version changes require an explicit wire-format
compatibility decision; ordinary releases do not change it.

Release preparation is intentionally manual and small: run the Rust checks,
run `./scripts/build-android.sh`, review generated bindings and native
libraries, update the application/core versions together when appropriate,
then create a `vX.Y.Z` tag and publish the release artifacts.

## License

Not yet decided.
