# HyprConnect Resource Guide

This is the study map for building HyprConnect: a Hyprland-native desktop companion that uses KDE Connect as the device/protocol backend and adds a daemon, CLI, and compositor-aware integrations for Hyprland users.

## Study Order

Read in this order:

1. KDE Connect architecture and DBus surface
2. KDE Connect packet/plugin model
3. Hyprland IPC and command/event model
4. Wayland desktop plumbing: clipboard, notifications, portals, media
5. Rust daemon and DBus implementation
6. Packaging and user-session integration

The goal is not to memorize everything. The goal is to know where each responsibility belongs.

## 1. KDE Connect Architecture

Start here to understand what HyprConnect should reuse instead of rebuilding.

- KDE Connect website: https://kdeconnect.kde.org/
- KDE Connect user docs: https://userbase.kde.org/KDEConnect/en
- KDE Connect source, official GitLab: https://invent.kde.org/network/kdeconnect-kde
- KDE Connect GitHub mirror: https://github.com/KDE/kdeconnect-kde
- KDE Connect Android source: https://invent.kde.org/network/kdeconnect-android

Local files to read in this checkout:

- `README.md`
- `daemon/kdeconnectd.cpp`
- `daemon/desktop_daemon.cpp`
- `core/daemon.h`
- `core/daemon.cpp`
- `core/device.h`
- `core/device.cpp`
- `core/pluginloader.h`
- `core/pluginloader.cpp`

Focus questions:

- What does KDE Connect core expose over DBus?
- What is handled by `kdeconnectd`?
- What belongs to a `Device`?
- How are plugins loaded and matched to remote capabilities?
- Which parts can HyprConnect call as a client instead of reimplementing?

Key design takeaway:

HyprConnect should initially be a client of KDE Connect core, not a replacement for KDE Connect core.

## 2. KDE Connect DBus

HyprConnect needs to talk to KDE Connect through DBus or `kdeconnect-cli`.

Official DBus learning resources:

- KDE DBus overview: https://develop.kde.org/docs/features/d-bus/
- KDE DBus introduction: https://develop.kde.org/docs/features/d-bus/introduction_to_dbus/
- KDE accessing DBus interfaces: https://develop.kde.org/docs/features/d-bus/accessing_dbus_interfaces/
- D-Bus specification: https://dbus.freedesktop.org/doc/dbus-specification.html

Local files to read:

- `cli/kdeconnect-cli.cpp`
- `dbusinterfaces/dbusinterfaces.h`
- `dbusinterfaces/dbusinterfaces.cpp`
- `daemon/org.kde.kdeconnect.service.in`

Runtime tools to learn:

```bash
busctl --user
busctl --user tree org.kde.kdeconnect
busctl --user introspect org.kde.kdeconnect /modules/kdeconnect
qdbus org.kde.kdeconnect
kdeconnect-cli --help
kdeconnect-cli --list-devices
kdeconnect-cli --list-available
```

Focus questions:

- What object paths does KDE Connect expose?
- Which methods already cover device listing, pairing, ringing, file sharing, and SMS?
- Which signals tell us when device state changes?
- Can the first MVP use `kdeconnect-cli`, or do we need direct DBus immediately?

Recommended approach:

Start with `kdeconnect-cli` for quick prototyping. Move to direct DBus once the workflows are proven.

## 3. KDE Connect Packet And Plugin Model

This is needed only after the DBus-client version is understood. Do not start here unless you intentionally want to build protocol-level compatibility.

Local files to read:

- `README.md`
- `core/networkpacket.h`
- `core/networkpacket.cpp`
- `core/networkpackettypes.h`
- `core/kdeconnectplugin.h`
- `core/kdeconnectplugin.cpp`
- `plugins/README.txt`

Important plugin READMEs:

- `plugins/battery/README`
- `plugins/clipboard/README`
- `plugins/findmyphone/`
- `plugins/mpriscontrol/README`
- `plugins/notifications/README`
- `plugins/ping/README`
- `plugins/share/README`
- `plugins/sftp/README`
- `plugins/systemvolume/README`
- `plugins/telephony/README`

Focus questions:

- What is a `NetworkPacket`?
- What packet types exist?
- Which plugins map directly to HyprConnect MVP features?
- What packet payloads are stable enough to depend on?

Feature mapping:

| HyprConnect Feature | KDE Connect Area |
| --- | --- |
| Device list | Core DBus / daemon |
| Battery | Battery plugin |
| Ring phone | Find My Phone plugin |
| Clipboard | Clipboard plugin |
| Send file | Share plugin |
| Remote media | MPRIS control plugin |
| Notifications | Notifications plugin |
| Commands from phone | Run command plugin |

## 4. KDE Connect Backends

Read this if you want to understand discovery, pairing, LAN, Bluetooth, and payload transport.

Local files to read:

- `core/backends/devicelink.h`
- `core/backends/devicelink.cpp`
- `core/backends/linkprovider.h`
- `core/backends/linkprovider.cpp`
- `core/backends/pairinghandler.h`
- `core/backends/pairinghandler.cpp`
- `core/backends/lan/lanlinkprovider.h`
- `core/backends/lan/lanlinkprovider.cpp`
- `core/backends/lan/landevicelink.h`
- `core/backends/lan/landevicelink.cpp`
- `core/backends/bluetooth/Multiplexing protocol.md`
- `core/backends/bluetooth/bluetoothlinkprovider.cpp`
- `core/backends/bluetooth/bluetoothdevicelink.cpp`

Focus questions:

- How does KDE Connect discover devices?
- How does pairing state work?
- How are payloads transferred?
- Which parts would be painful to reimplement?

Key design takeaway:

Discovery, pairing, encryption, and transport are expensive to get right. Reuse KDE Connect for these.

## 5. Hyprland IPC

This is the most important HyprConnect-specific area.

Official Hyprland docs:

- Hyprland IPC: https://wiki.hypr.land/IPC/
- Hyprland dispatchers: https://wiki.hypr.land/Configuring/Basics/Dispatchers/
- Hyprland variables: https://wiki.hypr.land/Configuring/Variables/
- Hyprland binds: https://wiki.hypr.land/Configuring/Binds/
- Hyprland rules: https://wiki.hypr.land/Configuring/Window-Rules/
- Hyprland plugin development: https://wiki.hypr.land/Plugins/Development/Getting-Started/

What to learn:

- `$HYPRLAND_INSTANCE_SIGNATURE`
- `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`
- `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock`
- `hyprctl -j monitors`
- `hyprctl -j clients`
- `hyprctl -j activewindow`
- `hyprctl dispatch ...`

Important warning:

The Hyprland command socket is synchronous. Open it, send one command, read the response, and close it. The event socket is the one to keep open.

Useful commands to prototype:

```bash
hyprctl -j monitors
hyprctl -j clients
hyprctl -j activewindow
hyprctl dispatch togglespecialworkspace phone
hyprctl dispatch exec hyprlock
hyprctl dispatch workspace name:phone
```

MVP Hyprland integrations:

- Phone command toggles a special workspace.
- Phone command locks the session.
- Phone command launches a configured app.
- Waybar module shows phone battery.
- Clipboard event from phone updates Wayland clipboard.
- File received from phone opens a notification with actions.

## 6. Wayland Clipboard

For MVP, use existing tools. Later, replace with native Rust Wayland clipboard code if needed.

- wl-clipboard: https://github.com/bugaevc/wl-clipboard
- wl-clipboard-rs: https://github.com/YaLTeR/wl-clipboard-rs
- Smithay clipboard crate: https://docs.rs/smithay-clipboard/latest/smithay_clipboard/

Commands to learn:

```bash
wl-copy "hello"
wl-paste
wl-paste --watch echo
```

Design rules:

- Clipboard sync must be opt-in.
- Desktop-to-phone sync should start as an explicit command, not automatic.
- Never log clipboard contents.
- Consider filtering obvious secrets later.

## 7. Notifications

For MVP, use Freedesktop notifications or a Rust wrapper. Avoid coupling to only one notification daemon.

References:

- Freedesktop notification spec: https://specifications.freedesktop.org/notification-spec/latest/
- notify-rust crate: https://docs.rs/notify-rust/latest/notify_rust/
- XDG portal notification API: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html

Notification daemons to test:

- mako: https://github.com/emersion/mako
- dunst: https://github.com/dunst-project/dunst
- swaync: https://github.com/ErikReider/SwayNotificationCenter

Tools:

```bash
notify-send "HyprConnect" "Test notification"
busctl --user tree org.freedesktop.Notifications
```

Focus questions:

- How do actions work in the notification spec?
- Can we attach actions like open file, reply, mute device?
- What varies between mako, dunst, and swaync?

## 8. Media Control

KDE Connect already has MPRIS-related plugins. HyprConnect may also expose convenient desktop-side controls.

References:

- MPRIS overview: https://specifications.freedesktop.org/mpris/latest/
- MPRIS Player interface: https://specifications.freedesktop.org/mpris/latest/Player_Interface.html
- playerctl: https://github.com/altdesktop/playerctl

Local files:

- `plugins/mpriscontrol/README`
- `plugins/mprisremote/`
- `dbusinterfaces/systeminterfaces/org.mpris.MediaPlayer2.xml`
- `dbusinterfaces/systeminterfaces/org.mpris.MediaPlayer2.Player.xml`

Commands:

```bash
playerctl -l
playerctl metadata
playerctl play-pause
busctl --user --list | grep org.mpris
```

## 9. Portals And Desktop Integration

Portals matter if HyprConnect later grows a GUI, file chooser, remote desktop features, screenshot features, or sandbox-friendly flows.

References:

- XDG Desktop Portal docs: https://flatpak.github.io/xdg-desktop-portal/docs/
- File chooser portal: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html
- Notification portal: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html
- Writing a portal backend: https://flatpak.github.io/xdg-desktop-portal/docs/writing-a-new-backend.html
- xdg-desktop-portal-hyprland: https://github.com/hyprwm/xdg-desktop-portal-hyprland

Local KDE Connect portal-related XML:

- `dbusinterfaces/systeminterfaces/org.freedesktop.portal.InputCapture.xml`
- `dbusinterfaces/systeminterfaces/org.freedesktop.portal.RemoteDesktop.xml`
- `dbusinterfaces/systeminterfaces/org.freedesktop.portal.Request.xml`
- `dbusinterfaces/systeminterfaces/org.freedesktop.portal.Session.xml`

## 10. Rust Stack

Recommended baseline:

- Tokio for async runtime.
- zbus for DBus.
- serde + toml for config.
- clap for CLI.
- tracing for logs.
- notify-rust for notifications.
- camino or std paths for path handling.

Links:

- Rust book: https://doc.rust-lang.org/book/
- Tokio tutorial: https://tokio.rs/tokio/tutorial
- zbus docs: https://docs.rs/zbus/latest/zbus/
- zbus book: https://dbus2.github.io/zbus/
- clap docs: https://docs.rs/clap/latest/clap/
- serde docs: https://serde.rs/
- toml crate: https://docs.rs/toml/latest/toml/
- tracing crate: https://docs.rs/tracing/latest/tracing/
- notify-rust: https://docs.rs/notify-rust/latest/notify_rust/

Crates to evaluate:

```toml
tokio = { version = "1", features = ["full"] }
zbus = { version = "5" }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
notify-rust = "4"
anyhow = "1"
thiserror = "2"
```

Daemon design topics to study:

- Async process execution with timeouts.
- Unix domain sockets.
- DBus signals.
- Long-running task cancellation.
- Structured logging.
- Config reload.
- Systemd user services.

## 11. Systemd User Service

HyprConnect should run as a user service.

References:

- systemd user services: https://wiki.archlinux.org/title/Systemd/User
- systemd service docs: https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html
- systemd unit docs: https://www.freedesktop.org/software/systemd/man/latest/systemd.unit.html

Commands:

```bash
systemctl --user daemon-reload
systemctl --user enable --now hyprconnect.service
systemctl --user status hyprconnect.service
journalctl --user -u hyprconnect.service -f
```

Service idea:

```ini
[Unit]
Description=HyprConnect daemon
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/hyprconnectd
Restart=on-failure

[Install]
WantedBy=default.target
```

## 12. Waybar, Rofi, Wofi, Swaync

These are likely the first integrations users will appreciate.

Waybar:

- Waybar repo: https://github.com/Alexays/Waybar
- Waybar custom module docs: https://github.com/Alexays/Waybar/wiki/Module:-Custom

Rofi / Wofi:

- rofi: https://github.com/davatorium/rofi
- wofi: https://hg.sr.ht/~scoopta/wofi

SwayNC:

- swaync: https://github.com/ErikReider/SwayNotificationCenter

Useful scripts to eventually ship:

- `hyprconnect-waybar`
- `hyprconnect-rofi-devices`
- `hyprconnect-rofi-send`
- `hyprconnect-swaync-actions`

Waybar output shape:

```json
{
  "text": "Pixel 82%",
  "tooltip": "Pixel 8: reachable, paired",
  "class": ["reachable", "charging"],
  "percentage": 82
}
```

## 13. Security And Privacy

Study this before implementing automatic sync.

Topics:

- Do not log clipboard contents.
- Do not log SMS contents.
- Make command execution explicit and configured.
- Never accept arbitrary command strings from the phone unless the user configured them.
- Restrict file receive locations.
- Make notification forwarding configurable per device.
- Show pairing/trust state clearly.

References:

- KDE Connect source and pairing code:
  - `core/backends/pairinghandler.cpp`
  - `core/kdeconnectconfig.cpp`
  - `core/device.cpp`
- Rust secure temp files: https://docs.rs/tempfile/latest/tempfile/
- Secret Service DBus API: https://specifications.freedesktop.org/secret-service/latest/

Command design rule:

Phone-triggered command packets should map to local command IDs:

```toml
[commands.lock]
label = "Lock"
exec = "hyprlock"
```

Not this:

```text
phone sends arbitrary shell command -> daemon executes it
```

## 14. Reference Projects

Study these for patterns, not for blind copying.

KDE Connect family:

- KDE Connect desktop: https://invent.kde.org/network/kdeconnect-kde
- KDE Connect Android: https://invent.kde.org/network/kdeconnect-android
- GSConnect: https://github.com/GSConnect/gnome-shell-extension-gsconnect

Hyprland ecosystem:

- Hyprland: https://github.com/hyprwm/Hyprland
- hyprland-rs: https://github.com/hyprland-community/hyprland-rs
- xdg-desktop-portal-hyprland: https://github.com/hyprwm/xdg-desktop-portal-hyprland

Wayland/desktop tooling:

- wl-clipboard: https://github.com/bugaevc/wl-clipboard
- playerctl: https://github.com/altdesktop/playerctl
- mako: https://github.com/emersion/mako
- dunst: https://github.com/dunst-project/dunst
- swaync: https://github.com/ErikReider/SwayNotificationCenter

## 15. First Prototype Checklist

Do this before building a full daemon:

1. Use `kdeconnect-cli --list-devices` and parse enough to show devices.
2. Use `kdeconnect-cli --ring <device>` or equivalent DBus call.
3. Use `kdeconnect-cli --share <file> --device <id>` or equivalent DBus call.
4. Use `hyprctl -j activewindow`.
5. Use Hyprland socket2 to print workspace events.
6. Use `notify-send` or `notify-rust` to show one notification.
7. Use `wl-copy` and `wl-paste` for clipboard push/pull.
8. Wrap these in one CLI command: `hyprconnect devices`.

Only after these work should we build the persistent daemon.

## 16. MVP Definition

The MVP should support:

```bash
hyprconnect devices
hyprconnect battery
hyprconnect ring <device>
hyprconnect send <path> <device>
hyprconnect clipboard push <device>
hyprconnect clipboard pull <device>
hyprconnect command lock
hyprconnect waybar
```

And this Hyprland binding:

```conf
bind = SUPER, P, exec, hyprconnect command phone_workspace
```

The first release does not need a GUI.

## 17. What To Avoid Early

Avoid these until the basic daemon is working:

- Reimplementing KDE Connect pairing.
- Reimplementing KDE Connect packet transport.
- Writing a Hyprland C++ plugin.
- Building a GUI first.
- Supporting every notification daemon perfectly.
- Auto-syncing clipboard by default.
- Adding SMS support before device/file/clipboard/notification basics.

## 18. Suggested Reading Sessions

Session 1:

- Read `README.md`.
- Read `cli/kdeconnect-cli.cpp`.
- Run `kdeconnect-cli --help`.
- Explore KDE Connect DBus with `busctl --user`.

Session 2:

- Read `core/device.*`.
- Read `core/daemon.*`.
- Read `core/pluginloader.*`.
- Skim `plugins/*/README`.

Session 3:

- Read Hyprland IPC docs.
- Write a small script that listens to `.socket2.sock`.
- Send a few `hyprctl dispatch` commands.

Session 4:

- Test `wl-copy`, `notify-send`, `playerctl`, and Waybar custom JSON.

Session 5:

- Start Rust workspace.
- Build `hyprconnect devices`.
- Build `hyprconnect waybar`.

