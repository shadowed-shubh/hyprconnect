# Phase 3 — Production Engineering & Release Infrastructure

## Completed

- Added one canonical Android build workflow at `scripts/build-android.sh`.
- Added deterministic tool, Android SDK/NDK, Rust target, linker, binding, and artifact checks to that workflow.
- Kept the existing checked-in Android `.so` policy and made regeneration explicit and reproducible.
- Added GitHub Actions CI for formatting, workspace checks, tests, Clippy, whitespace validation, and the canonical Android build.
- Added reusable test support for temporary configuration directories and loopback ephemeral TCP listeners.
- Added reusable loopback listener support for future integration tests; the default suite does not require local sockets.
- Added structured `tracing` output for daemon startup and pairing-confirmation timeout without logging key material or fingerprints.
- Documented current capabilities, prerequisites, build commands, CI, versioning, and release preparation.

## Build workflow

Run `./scripts/build-android.sh`. It builds `hyprconnect-core` for
`x86_64-linux-android` and `aarch64-linux-android`, generates Kotlin bindings
from `core/src/hyprconnect.udl`, synchronizes the repository and app binding
copies, copies the corresponding native libraries into `android/app/src/main/jniLibs/`,
and runs the Gradle wrapper's debug APK build.

The script defaults to NDK `30.0.16248370`. `ANDROID_SDK_ROOT`,
`ANDROID_NDK_HOME`, and `HYPRCONNECT_NDK_VERSION` can select an installed
environment without changing the source workflow.

## CI

`.github/workflows/ci.yml` runs on pushes and pull requests. It installs Rust
with Android targets and Clippy/rustfmt, configures JDK 17 and Android SDK/NDK,
runs all Rust quality checks, and invokes the same local Android script rather
than maintaining a second native build procedure.

## Testing

The core now has reusable test support for isolated temporary directories and
ephemeral loopback listeners. The default suite exercises the temporary
directory helper and keeps socket-based integration support available without
requiring sockets in every environment. Existing focused unit tests continue to cover protocol versions, pairing/connection cleanup,
PING/PONG routing, and unknown packet decoding.

Real mDNS, Android-device, and LAN reconnect tests remain intentionally
untested because they would make the default suite environment-dependent.

## Logging

The project continues to use `tracing`/`tracing-subscriber`. Daemon startup is
now a structured `info` event with device name and bind address fields, and
pairing timeout is explicitly logged. Existing discovery, handshake, session,
heartbeat, and pairing state transitions remain logged at appropriate
`info`/`warn` levels. Private keys, session keys, and fingerprint values are
not logged.

## Release workflow

The intended release sequence is:

1. Run formatting, check, tests, Clippy, and `git diff --check`.
2. Run `./scripts/build-android.sh` and review generated bindings, `.so` files,
   and the APK.
3. Keep protocol version `1` unless a separately reviewed wire-compatibility
   change is made.
4. Update the `0.1.x` core/daemon and Android versions when appropriate.
5. Create a `vX.Y.Z` tag and publish the reviewed artifacts.

## Decisions

- Native `.so` files remain tracked because the current Android project reads
  them directly from `jniLibs`; the build script is the reproducible way to
  refresh them.
- Both generated Kotlin copies remain tracked and are checked for byte identity.
- CI pins the Android NDK to `30.0.16248370`, matching the local build workflow.
- Protocol version `1` is unchanged. Core/daemon and Android remain at `0.1.0`.

## Remaining limitations

- Native libraries are debug/unstripped checked-in artifacts and make the APK large.
- The script assumes the Android NDK toolchain is installed locally or available through CI setup.
- UniFFI generation emits an optional ktlint warning when ktlint is not installed; generated output remains valid.
- Full mDNS/LAN reconnect and Android-device integration tests are not part of CI.
- HELLO-level protocol-version confirmation and other Phase 1/2 limitations remain future work.
