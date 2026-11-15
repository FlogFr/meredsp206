# Testing rules

Unit tests are the project's memory: they freeze every reverse-engineered fact
so a later change cannot silently break it. Maintain them on every request.

## Always

- Run `make check` (rustfmt, clippy with `-D warnings`, Rust tests, Python
  tests) before calling any change done. Report failures honestly, never skip
  or weaken a test to get green.
- **Every change ships with tests.** A new protocol fact, command, field,
  conversion or bug fix gets a test in the same change. A bug fix starts with a
  test that fails without the fix.
- When a test and the docs disagree, find out which one is wrong (re-check the
  capture or the disassembly). Fix the wrong one and say so to the user.
- Tests never touch the network or the real device (see `device-safety.md`).
  Use captured frames or, later, `MockDevice`.

## Where tests live

| What | Where | Run by |
|---|---|---|
| Unit tests of each module (frames, units, memory map, commands) | `#[cfg(test)] mod tests` in `crates/*/src/*.rs` | `cargo test` |
| Real-device reference vectors | `crates/dsp206-proto/tests/captures.rs` (reads `tools/re/captures/*.json`) | `cargo test` |
| Code ↔ docs ↔ tools consistency (command codes documented, read-only whitelist identical in code, `device-safety.md` and the probe) | `crates/dsp206-proto/tests/docs_sync.rs` | `cargo test` |
| Python tools (frame builder, whitelist, summary) | `tools/tests/test_probe.py` | `make test-python` |
| Docs hygiene (relative links, `CLAUDE.md` index complete, design PNG has a source, docs in English) | `tools/tests/test_docs.py` | `make test-python` |

## Writing good tests here

- Prefer **real captures** over invented values. When a new frame is captured
  on hardware, add it to `tools/re/captures/` and assert on it.
- Test the documented numbers literally (offsets, ranges, table entries), so
  that a doc edit and a code edit must agree.
- Keep tests dependency-free and fast (whole suite well under a few seconds).
  Pseudo-random data comes from a local xorshift helper, not a crate.
- Panicking (`unwrap`, `expect`) is fine in tests. `clippy.toml` allows it in
  `#[test]` functions, and integration test files allow it at the top.
