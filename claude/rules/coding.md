# Coding rules

Applies once implementation starts (see `claude/docs/architecture.md`).

## Rust

- Workspace under `crates/`. Respect the crate boundaries:
  `dsp206-proto` has no I/O, `dsp206-link` has no GUI, `dsp206-gtk` has no raw
  protocol code.
- MSRV 1.84. Only use dependencies packaged-friendly for Debian and keep the list
  short. Justify each new dependency in the commit message.
- No `unwrap()`/`expect()` in library code. Use typed errors (`thiserror`).
- Protocol constants and encodings come from `claude/docs/memory-map.md` and
  `claude/docs/protocol.md`. Cite the section in a short comment where a magic
  number appears.
- Before committing: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all`.

## Tests

- Every encoder/decoder has round-trip tests. Captured frames from
  `tools/re/captures/` are the reference vectors.
- Link-layer tests use `MockDevice`. Never the real device (see
  `device-safety.md`).

## Performance

- Idle CPU near zero: no busy loops. Redraw meters only when new data arrives and
  the view is visible.
- The release binary must stay small. Check `ls -lh target/release/` when adding
  dependencies.
