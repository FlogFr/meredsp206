# Documentation rules

All project documentation is written in **English**, even when the conversation
is in another language.

## Where things live

| Content | Location |
|---|---|
| Product and technical docs (protocol, memory map, architecture, RE notes) | `claude/docs/*.md` |
| Instructions for Claude | `claude/rules/*.md` |
| Desired product design | `design/*.png`, rendered from `design/src/*.html` |
| User documentation, build and install instructions | `README.md` |
| Index of everything above | `CLAUDE.md` |

## Keep them true

- **`CLAUDE.md` is updated at the end of every user request** that changes
  something it describes: new standing instruction, rule, command, directory,
  crate or project status. Record the user's standing instructions there (in
  English) so future sessions follow them.
- Update the relevant doc **in the same change** as the code or discovery it
  describes. A protocol fact used in code must be in `protocol.md` or
  `memory-map.md`.
- Tag every reverse-engineered fact: **[live]** (seen on the device), **[code]**
  (read in the original app) or **[guess]**. Upgrade the tag when a guess is
  confirmed, and fix or delete facts proven wrong.
- `README.md` describes only what exists. Planned features are clearly marked as
  planned.
- When the design changes, update `design/src/*.html`, re-render the PNGs with
  `design/render.sh`, and commit both.
- Add new docs to the index in `CLAUDE.md`.
- `tools/tests/test_docs.py` enforces part of this: relative links resolve,
  every file in `claude/docs/` and `claude/rules/` is linked from `CLAUDE.md`,
  every design PNG has an HTML source, and docs read as English.
