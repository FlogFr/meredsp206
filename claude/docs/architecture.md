# Architecture (planned)

Status: `dsp206-proto` is **implemented and tested**. `dsp206-link`,
`dsp206-cli` and `dsp206-gtk` are planned. Update this file whenever the design
changes.

## 1. Stack

| Concern | Choice | Why |
|---|---|---|
| Language | Rust 2021, MSRV 1.84 | Native, small and fast binaries, strong typing for a binary protocol. Toolchain present on the dev machine. |
| GUI | GTK 4 + libadwaita (`gtk4-rs`, `libadwaita-rs`) | Native look on Debian GNOME/Wayland. Runtime libraries already installed (`libgtk-4-1`, `libadwaita-1-0`), so the binary stays small. |
| Custom widgets | `GtkDrawingArea` + cairo (EQ curves, meters, faders) | No extra dependency. |
| Concurrency | `std::thread` + channels, no async runtime | One socket and one request in flight do not need tokio. |
| Serialization | `serde` + `serde_json` for native preset files | Readable, diff-friendly. |

## 2. Workspace layout

```
crates/
  dsp206-proto/   pure logic, no I/O: frames, commands, memory map, unit conversions
  dsp206-link/    TCP session (single in-flight request, timeouts, retries,
                  coalescing queue), UDP discovery, mock device for tests
  dsp206-cli/     `dsp206` command-line tool: discover, info, dump, meters, set
  dsp206-gtk/     GTK 4 / libadwaita application `meredsp206`
data/             .desktop file, AppStream metainfo, icons
```

### 2.1 `dsp206-proto` (implemented)

No dependencies, no I/O.

- `frame`: `Frame { src, dst, cmd, data }` with `encode()`/`decode()`, `checksum()`,
  and a streaming `FrameDecoder` (iterator) that cuts by length (no DLE
  stuffing), checks the trailer and XOR, and resynchronises after garbage.
- `command`: `Channel` (0–7), `Request` with one variant per documented command
  (`code()`, `payload()`, `frame(dev)`, `is_read_only()`, `reply_code()`,
  `coalesce_key()`), `READ_ONLY_CODES`, `ALL_CODES`, `Reply::parse()` and `Meters`.
- `program`: `Program` (header, name, 2 `InputChannel`, 6 `OutputChannel`,
  `Globals`) with lossless `from_bytes()`/`to_bytes()` over the 928 bytes,
  `to_pages()`, and `ProgramAssembler` to collect `0x24` pages.
- `units`: frequency, gains, thresholds, times, delay, Q/ratio/GEQ tables, slope
  and PEQ type labels, meter decoding.
- Tests: unit tests per module, `tests/captures.rs` (real frames), and
  `tests/docs_sync.rs` (code ↔ docs ↔ probe whitelist).

### 2.2 `dsp206-link`

- `Session::connect(addr)` owns the only TCP socket. A worker thread enforces one
  request in flight, an 800 ms timeout, 3 retries, and coalescing by
  `(cmd, ch, band)`.
- Emits events to the UI: `Connected(info)`, `ProgramLoaded`, `Meters`,
  `LinkDegraded`, `Disconnected`.
- Reconnect policy from `connection.md`: at least 60 s back-off after a close.
- `MockDevice` speaks the same protocol over a socket pair, so tests and demo mode
  need no hardware.

### 2.3 `dsp206-gtk`

- Main window: libadwaita header bar (device and preset menu, link status), a
  mixer view with 8 channel strips (meter, fader, mute, polarity, name), and a
  channel editor (EQ, crossover, dynamics, delay, routing).
- All edits update the local `Program` first, then queue the setter command.
  Undo/redo works on `Program` snapshots.
- Offline mode edits a `Program` with no device and can push it later.

See `design/*.png` for the target look.

## 3. Build, test and packaging

- `make check` runs rustfmt, clippy (`-D warnings`) and all Rust and Python tests.
- `cargo build --release` will produce `target/release/meredsp206` and `dsp206`.
- Build dependencies on Debian: `libgtk-4-dev`, `libadwaita-1-dev`, `pkg-config`.
- `make install PREFIX=/usr/local` installs binaries, the `.desktop` file and icons.
  A `.deb` can be produced later with `cargo-deb`.
