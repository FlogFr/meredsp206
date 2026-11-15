# CLAUDE.md

**meredsp206**: a native Linux app (Rust + GTK 4/libadwaita) to control a
t.racks DSP 206 loudspeaker processor over USB or the LAN. It replaces the Windows-only
*the t.racks DSP 206 Processor Editor V1.05*, whose protocol was reverse engineered
for this project.

Status (2026-11-15): first full implementation, awaiting the user's tests on hardware.
`dsp206-proto` (protocol, program, edits, files, USB HID framing), `dsp206-link`
(TCP and USB HID sessions, UDP and USB discovery, simulated device), `dsp206` CLI
and the `meredsp206` GTK app are written and tested against the simulated device.
The USB link (reverse engineered from the editor, protocol.md §1.2) works on the
real unit: CLI and GTK app read everything, meters run for 30 s without a miss,
and an approved Out6 mute/unmute was acknowledged (connection.md §3.2). The udev
rule is installed on the dev machine. Open issue: over the network the real unit
stops answering ~1–2 s after connect (connection.md); USB does not have it.

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

## Rules (instructions for you, always apply)

- [claude/rules/device-safety.md](claude/rules/device-safety.md): a real DSP is on
  the LAN. What you may send without asking, and what needs approval.
- [claude/rules/testing.md](claude/rules/testing.md): tests on every change,
  where each kind of test lives, `make check`.
- [claude/rules/documentation.md](claude/rules/documentation.md): docs in English,
  where each kind of doc lives, confidence tags, keep docs in sync.
- [claude/rules/coding.md](claude/rules/coding.md): Rust workspace conventions and
  performance.
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

## Repository layout

| Path | Content |
|---|---|
| `crates/dsp206-proto/` | Protocol crate: `frame`, `command`, `program`, `edit`, `units` modules, plus `tests/` (captures, docs sync) |
| `crates/dsp206-link/` | `Link` worker over TCP or USB HID (one request in flight, retries, coalescing), UDP and USB discovery, `MockDevice` |
| `crates/dsp206-cli/` | `dsp206` command-line tool |
| `crates/dsp206-gtk/` | `meredsp206` GTK 4 / libadwaita app (`model`, `format`, `response` are pure and tested) |
| `data/udev/` | udev rule giving the logged-in user access to the USB unit (`/dev/hidrawN`) |
| `tools/probe/` | Read-only device dump tool (`dsp206_dump.py`) |
| `tools/re/` | Reverse-engineering scripts and `captures/` (real frames, used as test vectors) |
| `tools/tests/` | Python tests for the tools and the docs |
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
```

Run the app: `cargo run -p dsp206-gtk -- --demo` (simulated unit), `-- --usb` or
`-- --connect 192.168.1.101`. CLI: `cargo run -p dsp206-cli -- help`
(`--usb` / `--hidraw PATH` select USB; `discover` lists USB and LAN units). The workspace uses `resolver = "3"` so
dependencies stay compatible with Rust 1.84.

Read-only device dump (wait 60 s between runs):
`python3 -I tools/probe/dsp206_dump.py -o dump.json`.
