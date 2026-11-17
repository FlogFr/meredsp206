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

## Kotlin (Android app, `android/`)

- See `claude/docs/android.md`. `core` is plain Kotlin/JVM: no Android API,
  testable with `./gradlew :core:test`. Android code lives in `app`.
- **The Kotlin port mirrors the Rust crates.** A protocol fact, command or
  encoding changed in `crates/dsp206-proto` or `crates/dsp206-link` changes in
  `android/core` in the same commit, with the same test.
- Compiler warnings are errors (`allWarningsAsErrors`), and Android lint
  warnings are errors. User-visible text goes in `res/values/strings.xml` with
  its French translation in `res/values-fr/strings.xml`.
- Keep dependencies to AndroidX and Compose. Versions live in
  `android/gradle/libs.versions.toml`.

## Python tools

- Standard library only, run with `python3 -I`. Tests in `tools/tests/` use
  `unittest`.

## Performance

- Idle CPU near zero: no busy loops. Redraw meters only when new data arrives and
  the view is visible.
- The release binary must stay small (`lto`, `strip` are on). Check
  `ls -lh target/release/` when adding dependencies.
