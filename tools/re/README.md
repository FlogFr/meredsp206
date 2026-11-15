# Reverse-engineering scripts

Helpers used to study *the t.racks DSP 206 Processor Editor V1.05*. The method
is described in `claude/docs/reverse-engineering.md`. Never commit the vendor
binary or its extracted content (see `claude/rules/reverse-engineering.md`).

## Setup (scratch directory, no root)

```bash
WORK=$(mktemp -d)                       # or your session scratch dir
cd "$WORK"
apt-get download innoextract libboost-program-options1.83.0
for d in *.deb; do dpkg -x "$d" root; done
LD_LIBRARY_PATH=root/usr/lib/x86_64-linux-gnu root/usr/bin/innoextract -d extract \
    /path/to/435192_the_t.racks_dsp_206_processor_editor_v1.05_setup.exe
mv "extract/app/the t.racks DSP 206 Processor Editor V1.05.exe" dsp206.exe
uv venv venv && uv pip install --python venv/bin/python pefile capstone unicorn
```

Run every script with `venv/bin/python -I <repo>/tools/re/<script>` from `$WORK`.

## Scripts

| Script | Purpose |
|---|---|
| `dump_res.py EXE OUTDIR` | Extract all PE resources (forms are `RT_RCDATA/T*`). |
| `dfm2txt.py FORM` | Binary DFM (`TPF0`) → text. Undoes the RTF obfuscation of the help form. |
| `rtf2txt.py FORM.txt` | Plain text of a form's `RichText` property. |
| `methods.py EXE > methods.txt` | Published VCL handler names → code addresses. |
| `fulldis.py EXE full.asm` | Linear disassembly of `.text`. |
| `dis.py ADDR…` | Disassemble functions (env `EXE`, `METHODS=methods.txt`), annotated with handler names and strings. |
| `funcof.py full.asm ADDR…` | Containing function of an address (heuristic). |
| `callgraph.py full.asm cg.pkl` | Build the call graph. `up.py cg.pkl ADDR…` / `down.py cg.pkl MAP` walk it. |
| `callargs.py full.asm TARGET…` | Show the pushed arguments at each call to TARGET. |
| `xref.py EXE VA…` | Raw data references to an address (vtables, tables). |
| `emu_parse.py EXE START END RAWOFF IDX0 NRAW` | Unicorn emulation of the page parser to map block offsets to struct fields. |
| `discover.py` | Send the Microchip UDP discovery probe and print replies. |

Example that reproduces the memory-map proof:

```bash
venv/bin/python -I tools/re/emu_parse.py dsp206.exe 419d8b 41aaab 3c6 2 950
```

## Captures

`captures/*.json` holds frames exchanged with a real unit (hex strings). They
serve as test vectors for `dsp206-proto`.
