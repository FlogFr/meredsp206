# CLAUDE.md

**meredsp206**: a native Linux app (Rust + GTK 4/libadwaita) to control a
t.racks DSP 206 loudspeaker processor over USB or the LAN. It replaces the Windows-only
*the t.racks DSP 206 Processor Editor V1.05*, whose protocol was reverse engineered
for this project. An Android app (Kotlin + Jetpack Compose, `android/`) controls the
same unit from a phone over Wi-Fi or USB OTG; the user wants to publish it on Google
Play and tests on a Motorola Moto G86.

Status (2026-11-15): first full implementation, awaiting the user's tests on hardware.
`dsp206-proto` (protocol, program, edits, files, USB HID framing), `dsp206-link`
(TCP and USB HID sessions, UDP and USB discovery, simulated device), `dsp206` CLI
and the `meredsp206` GTK app are written and tested against the simulated device.
The USB link (reverse engineered from the editor, protocol.md §1.2) works on the
real unit: CLI and GTK app read everything, meters run for 30 s without a miss,
and an approved Out6 mute/unmute was acknowledged (connection.md §3.2). The udev
rule is installed on the dev machine. Open issue: over the network the real unit
stops answering ~1–2 s after connect (connection.md); USB does not have it.
Preset store/recall had `0x20`/`0x21` swapped until 2026-11-15, so "store U01"
loaded U01 on the unit. Fixed from the original editor's state machine
(protocol.md §4.2). Stores into U01, U02, U19 and U20 then worked on the unit, but
the app read back the wrong name (`0x29` counts U01 from 0); fixed the same day,
the corrected read-back is not yet tried on hardware.

Android app (2026-11-17): first version written, see [claude/docs/android.md](claude/docs/android.md).
`android/core` is a Kotlin port of `dsp206-proto`, `dsp206-link` and the GTK model,
tested on the JVM with the same captures; `android/app` is the Compose UI (device,
mixer, channel editor, presets, log, demo). Builds, lint clean, Robolectric UI tests
pass against the simulated unit. Not yet tried on a phone or on the real unit.

## Working agreement (applies to every request)

1. **Keep this file current.** At the end of each user request, update
   `CLAUDE.md` whenever the request changed something it describes: a new rule or
   standing instruction from the user, a new command, a new directory or crate,
   a status change. New docs and rules must be linked below. `tools/tests/test_docs.py`
   checks the index.
2. **Maintain unit tests.** Every change comes with tests. Run `make check`
   before finishing, and report its real result. See
   [claude/rules/testing.md](claude/rules/testing.md).
3. Talk to the user in their language (French so far). Write every project file in
   English.
4. Never send state-changing commands to the real device without approval. See
   [claude/rules/device-safety.md](claude/rules/device-safety.md).
5. Keep the Android port in step: a protocol change in the Rust crates goes into
   `android/core` in the same commit, with its test (claude/rules/coding.md).

## Rules (instructions for you, always apply)

- [claude/rules/device-safety.md](claude/rules/device-safety.md): a real DSP is on
  the LAN. What you may send without asking, and what needs approval.
- [claude/rules/testing.md](claude/rules/testing.md): tests on every change,
  where each kind of test lives, `make check`.
- [claude/rules/documentation.md](claude/rules/documentation.md): docs in English,
  where each kind of doc lives, confidence tags, keep docs in sync.
- [claude/rules/coding.md](claude/rules/coding.md): Rust workspace and Kotlin
  (Android) conventions, performance.
- [claude/rules/reverse-engineering.md](claude/rules/reverse-engineering.md):
  handling vendor files, scratch dirs, recording findings.
- [claude/rules/design.md](claude/rules/design.md): how `design/*.png` is produced
  and the visual conventions.

## Product and technical docs (keep them current)

- [claude/docs/overview.md](claude/docs/overview.md): device, full feature
  inventory, goals.
- [claude/docs/protocol.md](claude/docs/protocol.md): transport, framing, command
  reference, meters.
- [claude/docs/memory-map.md](claude/docs/memory-map.md): 928-byte program layout
  and value encodings.
- [claude/docs/connection.md](claude/docs/connection.md): observed link quirks and
  the client rules they imply.
- [claude/docs/architecture.md](claude/docs/architecture.md): crates and stack,
  implemented and planned.
- [claude/docs/original-app.md](claude/docs/original-app.md): structure of the
  Windows editor and its key function addresses.
- [claude/docs/reverse-engineering.md](claude/docs/reverse-engineering.md): method,
  tools, what remains.
- [claude/docs/android.md](claude/docs/android.md): Android app: layout, why a
  Kotlin port, build and test, phone behaviour, Google Play checklist.

## Repository layout

| Path | Content |
|---|---|
| `crates/dsp206-proto/` | Protocol crate: `frame`, `command`, `program`, `edit`, `units` modules, plus `tests/` (captures, docs sync) |
| `crates/dsp206-link/` | `Link` worker over TCP or USB HID (one request in flight, retries, coalescing), UDP and USB discovery, `MockDevice` |
| `crates/dsp206-cli/` | `dsp206` command-line tool |
| `crates/dsp206-gtk/` | `meredsp206` GTK 4 / libadwaita app (`model`, `format`, `response` are pure and tested) |
| `android/core/` | Kotlin/JVM port: `dsp206.proto`, `dsp206.link` (worker, TCP, HID, discovery, `MockDevice`), `dsp206.editor` (model, session, format, response) |
| `android/app/` | Android app `io.github.flogfr.meredsp206` (Compose, Material 3), Robolectric UI tests |
| `.github/workflows/android.yml` | CI: Android tests, lint, debug APK artifact |
| `data/udev/` | udev rule giving the logged-in user access to the USB unit (`/dev/hidrawN`) |
| `data/` | Desktop entry `org.meredsp206.Editor.desktop` and app icon `icons/hicolor/` (installed by `make install`) |
| `tools/icon/` | `make_icons.py`: the app icon drawn once, written for Linux, Android and the Play Store |
| `android/store/` | Google Play listing assets (512 px icon) |
| `tools/probe/` | Read-only device dump tool (`dsp206_dump.py`) |
| `tools/re/` | Reverse-engineering scripts and `captures/` (real frames, used as test vectors) |
| `tools/tests/` | Python tests for the tools, the docs and the Rust ↔ Kotlin agreement |
| `design/` | Target UI mockups (`*.png`), HTML sources (`src/`), `render.sh` |
| `claude/` | Docs (`docs/`) and rules (`rules/`) for Claude |
| `README.md` | User documentation, build and install instructions |

## Key facts (details in the docs)

- TCP **9761**, one client at a time. Discovery on UDP **30303**.
- USB: HID `0168:0821` ("Musicrown Dsp Process"), one frame per 64-byte report,
  report ID 0. Input padding is **stale data**: always cut by the frame length.
  Linux node `/dev/hidrawN`, root-only until `data/udev/70-meredsp206.rules` is
  installed. USB is the reliable link: use it for live work.
- Frame: `10 02 src dst len cmd data… 10 03 xor`, `len = 1 + len(data)`, no DLE
  stuffing, XOR over all previous bytes. PC address 0, device ID default 1.
- Channels: 0 = InA, 1 = InB, 2–7 = Out1–Out6.
- Program = 928 bytes, read as 19 pages of 50 bytes with `0x27 [p]` → `0x24`.
- Presets: `0x20 [n]` **loads** (0 = F00, 1 = U01). Store = `0x26 [name]`,
  then `0x21 [n]` (n = U number), then `0x12`, each after the previous ACK.
  `0x29 [i]` and the `0x22` table count from 0: index `i` is U(i+1), and
  `0x29 [20]` is the current program name. `Request::GetPresetName(n)` takes
  the U number and does the shift.
- Keep one long-lived connection, one request in flight, 800 ms timeout.
  Over the network the device goes silent about 1–2 s after connect (not over
  USB), see [claude/docs/connection.md](claude/docs/connection.md).

## Commands

```bash
make check          # everything below: fmt check, clippy -D warnings, all tests
make test           # Rust (cargo test --workspace --all-targets) + Python tests
make test-rust
make test-python    # python3 -I -m unittest discover -s tools/tests
make lint
make fmt
make design         # re-render design/*.png from design/src/*.html
make android-core   # Kotlin core tests on the JVM (JDK 17+)
make android-check  # + Robolectric UI tests and Android lint (needs the Android SDK)
make android-apk    # debug APK: android/app/build/outputs/apk/debug/app-debug.apk
make icons          # regenerate every app icon from tools/icon/make_icons.py
sudo make install PREFIX=/usr/local   # after cargo build --release: binaries, desktop entry, icon
```

Android SDK in a cloud session: the `meredsp206` environment allows `dl.google.com`,
`maven.google.com` and `gcp.mirrors.maven.org`. Install `platforms;android-37.0`
and `build-tools;36.0.0` with `sdkmanager` (the user accepted the SDK licence on
2026-11-17) and write `sdk.dir=...` to `android/local.properties`.

Run the app: `cargo run -p dsp206-gtk -- --demo` (simulated unit), `-- --usb` or
`-- --connect 192.168.1.101`. CLI: `cargo run -p dsp206-cli -- help`
(`--usb` / `--hidraw PATH` select USB; `discover` lists USB and LAN units). The workspace uses `resolver = "3"` so
dependencies stay compatible with Rust 1.84.

Read-only device dump (wait 60 s between runs):
`python3 -I tools/probe/dsp206_dump.py -o dump.json`.
