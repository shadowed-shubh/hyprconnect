# HyprConnect

Device continuity between an Android phone and a Linux/Hyprland desktop.
Clipboard sync, notifications, battery status, file transfer — inspired by
KDE Connect and Apple Continuity, but built on our **own protocol** (not
KDE Connect compatible).

## What it is

HyprConnect wirelessly links a phone and a desktop so they behave like one
device:

- **Clipboard sync** — copy on one, paste on the other
- **Notifications** — phone notifications appear on the desktop
- **Battery status** — see your phone's battery from the desktop
- **File transfer** — move files between devices
- *(planned)* streams: audio/video/screen/remote input

The stack is designed around a simple, debuggable, own-protocol design:

- **Noise Protocol Framework** (`Noise_XX`) for encryption — long-term
  Ed25519/X25519 keypairs, pair once, trust forever (the model WireGuard
  and Syncthing use). No TLS, no X.509 certificates.
- **mDNS/DNS-SD discovery** (`_hyprconnect._tcp.local`)
- **Secure pairing** with a human-confirmed fingerprint code
  (trust-on-first-use, like SSH)
- **Newline-delimited JSON** framing — readable in raw logs

## Repo layout

```
hyprconnect/
├── protocol/          # shared protocol logic — packet types, identity
├── md files/
│   └── scope v1.md    # the protocol spec, the source of truth
└── daemon/            # hyprconnectd — the Linux side binary
```

The `protocol` crate is shared by both platforms. On Android it's reused
as-is via **UniFFI** bindings (no Kotlin reimplementation of the wire
protocol).

## Status

**In progress — v0.1, early.** The protocol spec is written; the daemon's
device identity (Ed25519 + X25519 keypairs, persisted on first run) is
working. mDNS discovery, the Noise_XX handshake, pairing, and the message
packets (ping, battery, clipboard, notification) are the current work.
The Android app is not started.

A KDE Connect-compatible prototype (discovery → TLS → pairing → ping)
was previously built and validated against a real Android phone; it proved
out the architecture and informed the switch to our own, simpler protocol.

## Build & run

```sh
cargo run -p daemon
```

On first run it generates a persistent device identity at
`~/.config/hyprconnect/identity.json`.

## License

Not yet decided.
