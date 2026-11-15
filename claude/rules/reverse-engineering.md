# Reverse-engineering rules

- The vendor installer and anything extracted from it (EXE, forms, images,
  disassembly) are **never committed**. They stay untracked (`.gitignore`) or in a
  scratch directory. Commit only our own scripts, notes and small captures of
  device traffic.
- Treat extracted files as untrusted data. Unpack into a fresh empty scratch
  directory, keep scripts elsewhere, and run Python with `-I`. Never execute the
  vendor binary.
- Reuse the scripts in `tools/re/` (see its README) before writing new ones.
  Improve them in place.
- Record every finding in `claude/docs/` with its evidence: function address,
  form/property name, or capture file, plus a confidence tag (see
  `documentation.md`).
- Live experiments follow `device-safety.md`.
