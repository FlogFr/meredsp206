# CLAUDE.md

**meredsp206**: a native Linux app (Rust + GTK 4/libadwaita) to control a
t.racks DSP 206 loudspeaker processor over the LAN. It replaces the Windows-only
*the t.racks DSP 206 Processor Editor V1.05*, whose protocol was reverse engineered
for this project.

Status: reverse engineering done (protocol and memory map documented). The app
itself is not implemented yet.

## Read before working

Rules (instructions for you, always apply):

- [claude/rules/device-safety.md](claude/rules/device-safety.md): a real DSP is on
  the LAN. What you may send without asking, and what needs approval.
- [claude/rules/documentation.md](claude/rules/documentation.md): docs in English,
  where each kind of doc lives, confidence tags, keep docs in sync.
- [claude/rules/coding.md](claude/rules/coding.md): Rust workspace conventions,
  tests, performance.
- [claude/rules/reverse-engineering.md](claude/rules/reverse-engineering.md):
  handling vendor files, scratch dirs, recording findings.
- [claude/rules/design.md](claude/rules/design.md): how `design/*.png` is produced
  and the visual conventions.

Product and technical docs (keep them current):

- [claude/docs/overview.md](claude/docs/overview.md): device, full feature
  inventory, goals.
- [claude/docs/protocol.md](claude/docs/protocol.md): transport, framing, command
  reference, meters.
- [claude/docs/memory-map.md](claude/docs/memory-map.md): 928-byte program layout
  and value encodings.
- [claude/docs/connection.md](claude/docs/connection.md): observed link quirks and
  the client rules they imply.
- [claude/docs/architecture.md](claude/docs/architecture.md): planned crates and
  stack.
- [claude/docs/original-app.md](claude/docs/original-app.md): structure of the
  Windows editor and its key function addresses.
- [claude/docs/reverse-engineering.md](claude/docs/reverse-engineering.md): method,
  tools, what remains.

Other locations:

- `README.md`: user documentation, plus build and install instructions.
- `design/`: target UI mockups (`*.png`), their HTML sources (`src/`) and
  `render.sh`.
- `tools/re/`: reverse-engineering scripts. `tools/probe/`: read-only device probe.

## Key facts (details in the docs)

- TCP **9761**, one client at a time. Discovery on UDP **30303**.
- Frame: `10 02 src dst len cmd data… 10 03 xor`, `len = 1 + len(data)`, no DLE
  stuffing, XOR over all previous bytes. PC address 0, device ID default 1.
- Channels: 0 = InA, 1 = InB, 2–7 = Out1–Out6.
- Program = 928 bytes, read as 19 pages of 50 bytes with `0x27 [p]` → `0x24`.
- Keep one long-lived connection, one request in flight, 800 ms timeout.
  Reconnecting quickly makes the device go silent for about a minute.

## Commands

No code yet. Once the workspace exists (see `claude/docs/architecture.md`):

```bash
cargo build --release
cargo test --all
cargo clippy --all-targets -- -D warnings
```

Render design mockups: `design/render.sh`.
