# Reverse-engineering notes

How the facts in [protocol.md](protocol.md) and [memory-map.md](memory-map.md) were
obtained, so they can be checked and extended.

## 1. Inputs

- Vendor installer (untracked, git-ignored):
  `435192_the_t.racks_dsp_206_processor_editor_v1.05_setup.exe`.
- A real DSP 206 on the LAN at `192.168.1.101` (firmware `DSP206 V0105P`).

## 2. Toolchain (no root needed)

| Step | Tool |
|---|---|
| Unpack Inno Setup | `innoextract` (`apt-get download innoextract libboost-program-options1.83.0`, then `dpkg -x` into a scratch dir) |
| PE parsing, resources | Python `pefile` in a scratch virtualenv |
| Disassembly | Python `capstone` (linear sweep of `.text`, plus recursive per-function) |
| Emulation | Python `unicorn` (runs a parser function on marker data) |
| Device probing | Python sockets (`tools/probe/`) |

All helper scripts live in `tools/re/` (see its README). Run them from a scratch
directory, never inside the extracted vendor files.

## 3. Method

1. **Forms first.** `dump_res.py` extracts the 40 binary DFM forms. `dfm2txt.py`
   converts them to text, giving every widget, its range, caption and event
   handler name. The RS-232 help form's rich text is obfuscated by adding
   `0x0E00` to every UTF-16 code unit. The converter undoes it.
2. **Published methods.** `methods.py` finds the VCL method tables
   (`u16 size, u32 code, shortstring name`), mapping 348 handler names to code
   addresses.
3. **Transport.** WinSock imports by ordinal (`socket`, `connect`, `send`, `recv`,
   `WSAAsyncSelect`) point to `TClientSocket`. `FormCreate` sets port `0x2621`
   (9761). Following `SendBuf` leads to the transport thread and the frame
   builder `0x412d34`.
4. **Commands.** Every message type has a constructor that stores the command byte
   at offset 5 of the message object (`mov byte [reg+5], imm`). `callgraph.py`,
   `up.py` and `down.py` link those constructors to UI handlers.
5. **Memory map.** `emu_parse.py` runs the core of the page parser
   (`0x419d8b..0x41aaab`) under Unicorn twice, with distinct marker bytes, and
   diffs the program struct. The result is an identity mapping, so the struct
   layout from the parser's stores *is* the device block layout. Senders that
   write into the struct (gain at `+0x94`, mute masks at `0x380`, …) name the
   fields.
6. **Units.** Slider ranges come from the DFM. Lookup tables (Q, ratio, GEQ bands)
   were found by scanning `.data` for monotonic float/double arrays. The
   frequency formula was read from `0x4214d0` (`1000 · 1.0233739^(i − 170)`).
   Delay and meter constants were read from the FPU code (96.0, 343.5, 3.2808,
   20·log10 + 18).
7. **Live check.** Read-only probes (`connect`, `identify`, preset names, program
   pages, meters) confirmed framing, checksum, addressing and the memory map
   against the factory default program.

## 4. What remains

See "Open questions" in [protocol.md](protocol.md#5-open-questions). Suggested
order:

1. Over USB the unit answers every request (protocol.md §1.2), so use USB for
   the next live work. The network reply loss ([connection.md](connection.md))
   stays unexplained.
2. Capture one frame per setter by changing a single parameter on a scratch
   preset (user approval needed, see `claude/rules/device-safety.md`).
3. Decode the store/recall/upload/download ACK state machine (`0x422bac` case
   `0x01`, states in `this+0x22c4`).
4. Decode `.prs` / `.unt` file formats (`OpenClick` `0x44556c`, `SaveClick`
   `0x445728`).
