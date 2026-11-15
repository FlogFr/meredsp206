# Coding rules

See `claude/docs/architecture.md` for the crates and `testing.md` for tests.

## Rust

- Workspace under `crates/`. Respect the crate boundaries:
  `dsp206-proto` has no I/O and no dependencies, `dsp206-link` has no GUI,
  `dsp206-gtk` has no raw protocol code.
- MSRV 1.84 (`rust-version` in `Cargo.toml`). Only use dependencies that are
  Debian-friendly, and keep the list short. Justify each new dependency in the
  commit message.
- No `unwrap()`/`expect()` in library code (denied by workspace lints). Return
  typed errors: hand-written enums implementing `Display` and `Error`, as in
  `dsp206-proto`. `thiserror` is acceptable in crates that already have
  dependencies.
- `unsafe` is forbidden workspace-wide.
- Protocol constants and encodings come from `claude/docs/memory-map.md` and
  `claude/docs/protocol.md`. Cite the section in a short comment where a magic
  number appears.
- Before finishing: `make check` (runs `cargo fmt --check`, `cargo clippy -D warnings`
  and every test).

## Python tools

- Standard library only, run with `python3 -I`. Tests in `tools/tests/` use
  `unittest`.

## Performance

- Idle CPU near zero: no busy loops. Redraw meters only when new data arrives and
  the view is visible.
- The release binary must stay small (`lto`, `strip` are on). Check
  `ls -lh target/release/` when adding dependencies.
