# Device safety

A real DSP 206 drives real speakers on the user's LAN (default `192.168.1.101`,
TCP 9761). Treat it like production hardware.

## Allowed without asking

- UDP discovery on port 30303.
- Read-only commands: `0x10` connect, `0x11` disconnect, `0x13` identify, `0x14`,
  `0x22`, `0x29`, `0x27` (program pages), `0x2c`, `0x40` (meters).
- At most **one** probe session at a time, and at least **60 s** between two
  sessions (see `claude/docs/connection.md`).

## Ask the user first, every time

Anything that changes sound or device state, even "harmless" values:

- All setters (`0x30`–`0x3f`, `0x41`, `0x48`, `0x49`, `0x15`, `0x2a`).
- Preset store/recall (`0x20`, `0x21`, `0x26`), page writes (`0x1a`, `0x25`).
- Lock/PTS (`0x2d`–`0x2f`), device ID/IP changes, factory reset.
- Any command whose meaning is not confirmed yet.

When asking, state the exact frame(s), the audible effect, and how to undo it.
Prefer changes that lower risk: mute or lower gain before anything else, never raise
gain, and test on an unused output when possible.

## Always

- Use the mock device (`dsp206-link::MockDevice`) for automated tests. Tests must
  never touch the network.
- Never loop reconnects. Never run background pollers against the device and leave
  them running.
- Log every frame sent to the real device during experiments. Store useful
  captures under `tools/re/captures/` with a short description.
