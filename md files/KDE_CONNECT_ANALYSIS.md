# KDE Connect Architecture Analysis

Generated from a full graph extraction of the KDE Connect KDE codebase (`kdeconnect-kde`) on 2026-05-14.

## Project Overview

KDE Connect bridges devices (phones, tablets, TVs, laptops) over your local network. It is a mature KDE project with 1772 source-level nodes, 2337 relationships, and 218 functional communities.

## Architecture (4 Layers)

```
LinkProvider (discovery)
  → DeviceLink (transport)
    → Device (identity + pairing + plugins)
      → Daemon (device manager)
```

### 1. LinkProvider — Discovery Layer

Three backends discovered by the graph:

| Backend | Discovery Mechanism | Key Node |
|---------|-------------------|----------|
| LAN | mDNS (Avahi) | `mDNS Network Discovery` community (69 nodes) |
| Bluetooth | RFCOMM + multiplexed channels | `Bluetooth Backend` community (61 nodes) |
| Loopback | Local testing | `LoopbackDeviceLink` |

**Key graph finding**: The mDNS discovery community has a cohesion score of 0.07, indicating weak internal coupling. This is a good target for hyprconnect to replace or supplement with a lighter discovery mechanism if needed.

### 2. DeviceLink — Transport Layer

Each link carries `NetworkPacket` (JSON-like key-value payloads). Packet types include:
- `PACKET_TYPE_PAIR` — pairing handshake
- `PACKET_TYPE_IDENTITY` — device identity exchange
- Application-specific types per plugin

All three `DeviceLink` implementations (`LanDeviceLink`, `BluetoothDeviceLink`, `LoopbackDeviceLink`) extend a common `DeviceLink` base.

### 3. Device — Identity + Pairing + Plugins

The `Device` class is the central abstraction (highest degree in the graph). Each device has:

- **Identity**: `DeviceInfo` with name, type, protocol version
- **Certificate**: SSL certificate stored in `KdeConnectConfig`
- **PairingHandler**: State machine managing trust
- **Plugins**: Loaded per-device based on capability negotiation

#### Pairing Flow (from source)

```
requestPairing()
  → PairingHandler sends PACKET_TYPE_PAIR { pair: true, timestamp }
  → Peer receives → PairingHandler state → RequestedByPeer
  → DesktopDaemon shows popup (askPairingConfirmation())
  → User accepts → acceptPairing() sends { pair: true }
  → pairingDone() → state = Paired → plugins load
```

Security: `verificationKey()` = SHA-256(both certs' public keys + timestamp), truncated to 8 hex chars for out-of-band verification.

**Key graph finding**: `disconnect()` is the single most cross-cutting function in the codebase (highest betweenness centrality, 20 edges). It bridges Bluetooth transport, SMS conversations, notifications, and device management. This means the unpair/disconnect path touches nearly everything — important for hyprconnect to handle gracefully.

### 4. Daemon — Device Manager

`DesktopDaemon` (extends `Daemon`) manages the device registry, exposes DBus interfaces (`org.kde.kdeconnect.device`), and handles pairing UI.

**Key graph finding**: The `Plugin Architecture` community (38 nodes) aggregates ~30 plugins. Each plugin links to `kdeconnectcore` (core shared library) and uses DBus for IPC.

## Plugin System (30+ plugins)

| Category | Plugins |
|----------|---------|
| **Notifications** | `sendnotifications`, `notifications` (receive + forward) |
| **Clipboard** | `clipboard` (sync) |
| **Media Control** | `mpriscontrol`, `mprisremote` (local ↔ remote) |
| **Input Sharing** | `mousepad`, `shareinputdevices`, `shareinputdevicesremote` |
| **Telephony** | `telephony`, `sms`, `mmtelephony` |
| **File Transfer** | `share`, `sftp` |
| **System** | `lockdevice`, `findmyphone`, `findthisdevice`, `runcommand`, `remotecommands`, `battery`, `pausemusic`, `presenter`, `screensaver-inhibit` |

**Key graph finding**: The graph identified a `Local and Remote Control Plugin Pairs` hyperedge — plugins like `mpriscontrol/mprisremote`, `systemvolume/remotesystemvolume`, `runcommand/remotecommands` follow a consistent pattern where a local plugin and a remote plugin communicate symmetrically. This pattern is directly useful for hyprconnect.

## DBus Surface

The primary integration surface for hyprconnect:

| Interface | Purpose |
|-----------|---------|
| `org.kde.kdeconnect.daemon` | List devices, pairing state |
| `org.kde.kdeconnect.device` | Per-device operations (send packet, plugins) |
| `org.kde.kdeconnect.plugin.*` | Per-plugin interfaces |

**Note**: `kdeconnectd` is launched via systemd user service and exposes these DBus interfaces. hyprconnect can connect to them without any KDE desktop components.

## Graph-Discovered Design Patterns Relevant to hyprconnect

1. **Common Plugin Architecture**: All plugins share `kdeconnectcore`, Qt DBus, KF6 i18n, and config QML. hyprconnect's modules (notifications, clipboard, media, file_transfer) mirror this pattern exactly.

2. **Notification Send/Receive Bridge**: The graph found a specific hyperedge pairing `notifications` and `sendnotifications` plugins. hyprconnect's notification bridge maps directly to `sendnotifications` plugin interface.

3. **Workspace Actions**: The graph shows that `remotecommands` plugin already supports executing local commands from phone triggers. hyprconnect just needs to wrap this interface — the protocol exists.

4. **Clipboard**: The `clipboard` plugin already handles bidirectional clipboard. The graph confirms `wl-copy`/`wl-paste` synergy is KDE Connect agnostic — the plugin just sends/receives text payloads over `NetworkPacket`.

## Integration Path for hyprconnect

Based on the graph analysis, the most efficient build order:

1. **DBus client module** — talk to `org.kde.kdeconnect.daemon` for device list and state
2. **Device listing + battery** — read-only, zero protocol knowledge needed
3. **File send** — one-shot DBus call, well-documented
4. **Command bridge** — wrap `remotecommands` plugin interface
5. **Notification listener** — subscribe to `sendnotifications` plugin signals
6. **Clipboard bridge** — wrap `clipboard` plugin interface
7. **Hyprland event socket** — independent of KDE Connect, can be developed in parallel

## Warnings from the Graph

- **60 isolated nodes** found by clustering — these are likely plugins or components with missing semantic relationships. Some may be dead code or experimental. Don't assume everything in KDE Connect is equally maintained.
- **Cohesion scores**: Most communities score 0.07–0.15 (low). KDE Connect has loose internal coupling. This makes it easy to talk to individual pieces without understanding the whole — good for incremental integration.

---

*Generated by graphify from `kdeconnect-kde` — 1772 nodes, 2337 edges, 218 communities clustered.*
