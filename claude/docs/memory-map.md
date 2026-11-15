# DSP 206 program memory map

The DSP's **current program** (and each stored preset) is a 928-byte block
(`0x000`–`0x39F`). The original editor's in-memory program struct is a byte-for-byte
copy of that block. This was proven by emulating its page parser (function
`0x419cd8`) with marker bytes: every block offset `k` lands in struct offset `k`.

All multi-byte values are **little-endian u16**. Offsets below are absolute block
offsets. Tags: **[live]** seen in a real dump, **[code]** read from the app,
**[guess]** inferred.

## 1. Paging

- Read the current program with `0x27 [p]` for `p = 0…18`. Each reply is
  `0x24 [p, 50 bytes]`. Page `p` carries block bytes `[50·p, 50·p + 50)` [live][code].
- 19 pages × 50 bytes = 950 bytes. Bytes 928–949 are padding.
- The original app parses the block only once page 18 has arrived [code].

## 2. Layout overview

| Range | Size | Content |
|---|---|---|
| `0x000`–`0x00F` | 16 | Header and preset name |
| `0x010`–`0x09B` | 140 | Input A (`ch` 0) |
| `0x09C`–`0x127` | 140 | Input B (`ch` 1) |
| `0x128`–`0x37F` | 6 × 100 | Outputs 1–6 (`ch` 2–7) |
| `0x380`–`0x39F` | 32 | Global masks |

### 2.1 Header

| Offset | Type | Field | Tag |
|---|---|---|---|
| `0x000` | u8 | Unknown (observed `0xFF`) | [live] |
| `0x001` | u8 | Unknown (observed `0x00`) | [live] |
| `0x002` | 14 × char | Program/preset name, ASCII, NUL padded (`Default Preset`) | [live][code] |

### 2.2 Input channel (InA base `0x010`, InB base `0x09C`, stride `0x8C`)

| Rel | InA abs | Type | Field | Default | Tag |
|---|---|---|---|---|---|
| `+0x00` | `0x010` | 8 × char | Channel name | `InA` / `InB` | [live] |
| `+0x08` | `0x018` | u16 | Gate attack | 0 (1 ms) | [live][code] |
| `+0x0A` | `0x01A` | u16 | Gate hold | 99 (100 ms) | [live][code] |
| `+0x0C` | `0x01C` | u16 | Gate release | 99 (100 ms) | [live][code] |
| `+0x0E` | `0x01E` | u16 | Gate threshold | 0 (−90 dB) | [live][code] |
| `+0x10` | `0x020` | 31 × u16 | GEQ band gains (bands of §4.3) | 120 (0 dB) | [live][code] |
| `+0x4E` | `0x05E` | 8 × PEQ | PEQ bands 1–8 (§2.4) | see §5 | [live][code] |
| `+0x7E` | `0x08E` | u16 | High-pass frequency index | 0 (19.7 Hz) | [live][code] |
| `+0x80` | `0x090` | u16 | Low-pass frequency index | 300 (20.16 kHz) | [live][code] |
| `+0x82` | `0x092` | u8 | High-pass slope | 0 | [live][code] |
| `+0x83` | `0x093` | u8 | Low-pass slope | 0 | [live][code] |
| `+0x84` | `0x094` | u16 | Gain | 280 (0 dB) | [live][code] |
| `+0x86` | `0x096` | u16 | Polarity (0 normal, 1 inverted) | 0 | [live][code] |
| `+0x88` | `0x098` | u16 | Delay (samples at 96 kHz) | 0 | [live][code] |
| `+0x8A` | `0x09A` | u16 | Link mask | own bit (InA 1, InB 2) | [live][code] |

### 2.3 Output channel (Out1 base `0x128`, stride `0x64`)

Bases: Out1 `0x128`, Out2 `0x18C`, Out3 `0x1F0`, Out4 `0x254`, Out5 `0x2B8`, Out6 `0x31C`.

| Rel | Out1 abs | Type | Field | Default | Tag |
|---|---|---|---|---|---|
| `+0x00` | `0x128` | 8 × char | Channel name | `Out1` … `Out6` | [live] |
| `+0x08` | `0x130` | u16 | Matrix routing mask (bit 0 InA, bit 1 InB) | 1 on Out1/Out3 | [live][code] |
| `+0x0A` | `0x132` | u16 | Mix gain from InA (gain scale, max 280) | 280 | [live][code] |
| `+0x0C` | `0x134` | u16 | Mix gain from InB | 280 | [live][code] |
| `+0x0E` | `0x136` | u16 | High-pass frequency index | 0 | [live][code] |
| `+0x10` | `0x138` | u16 | Low-pass frequency index | 300 | [live][code] |
| `+0x12` | `0x13A` | u8 | High-pass slope | 0 | [live][code] |
| `+0x13` | `0x13B` | u8 | Low-pass slope | 0 | [live][code] |
| `+0x14` | `0x13C` | 9 × PEQ | PEQ bands 1–9 (§2.4) | see §5 | [live][code] |
| `+0x4A` | `0x172` | u16 | Compressor ratio index | 0 (1:1.0) | [live][code] |
| `+0x4C` | `0x174` | u16 | Compressor attack | 49 (50 ms) | [live][code] |
| `+0x4E` | `0x176` | u16 | Compressor release | 499 (500 ms) | [live][code] |
| `+0x50` | `0x178` | u16 | Compressor knee (dB) | 0 | [live][code] |
| `+0x52` | `0x17A` | u16 | Compressor threshold | 220 (+20 dB) | [live][code] |
| `+0x54` | `0x17C` | u16 | Limiter attack | 49 (50 ms) | [live][code] |
| `+0x56` | `0x17E` | u16 | Limiter release | 499 (500 ms) | [live][code] |
| `+0x58` | `0x180` | u16 | Limiter knee (dB) | 0 | [live][code] |
| `+0x5A` | `0x182` | u16 | Limiter threshold | 220 (+20 dB) | [live][code] |
| `+0x5C` | `0x184` | u16 | Gain | 280 (0 dB) | [live][code] |
| `+0x5E` | `0x186` | u16 | Polarity | 0 | [live][code] |
| `+0x60` | `0x188` | u16 | Delay (samples at 96 kHz) | 0 | [live][code] |
| `+0x62` | `0x18A` | u16 | Link mask (bit k = Out k+1) | own bit | [live][code] |

### 2.4 PEQ band record (6 bytes)

| Rel | Type | Field | Tag |
|---|---|---|---|
| `+0` | u16 | Gain (§4.2) | [live][code] |
| `+2` | u16 | Frequency index (§4.1) | [live][code] |
| `+4` | u8 | Q index (§4.4) | [live][code] |
| `+5` | u8 | Filter type (§4.5) | [live][code] |

Per-band bypass flags are **not** in the record. They live in the global masks (§2.5).

### 2.5 Global block

| Offset | Type | Field | Tag |
|---|---|---|---|
| `0x380` | u16 | Input mute mask (bit 0 InA, bit 1 InB) | [code] |
| `0x382` | u16 | Output mute mask (bit k = Out k+1). Read back as `0x20` after muting Out6 with `0x35`, `0` after unmuting | [live][code] |
| `0x384` | 2 × u16 | Input PEQ band-bypass masks (bit n = band n+1) | [code] |
| `0x388` | 6 × u16 | Output PEQ band-bypass masks | [code] |
| `0x394` | u16 | Unknown (read in many places; HPF/LPF bypass or lock?) | [guess] |
| `0x396` | u16 | Unknown | [guess] |
| `0x398` | u16 | Delay display unit (0 ms, 1 m, 2 ft) | [code][guess] |
| `0x39A` | u16 | Input PEQ bypass mask (whole EQ) | [code] |
| `0x39C` | u16 | Output PEQ bypass mask | [code] |
| `0x39E` | u16 | Input GEQ bypass mask | [code] |

## 3. Channel indices

`ch` in commands: 0 = InA, 1 = InB, 2–7 = Out1–Out6. Masks use separate bit spaces
for inputs (bits 0–1) and outputs (bits 0–5) [code].

## 4. Value encodings

### 4.1 Frequency index (PEQ, HPF, LPF)

`raw` 0…300, 1/30-octave steps, `raw = 170` is 1 kHz [code]:

```
f(raw) = 1000 · 2^((raw − 170) / 30)  Hz        raw 0 → 19.7 Hz, raw 300 → 20.16 kHz
raw(f) = round(30 · log2(f / 1000) + 170)
```

### 4.2 Gains

| Quantity | Raw range | Conversion | Tag |
|---|---|---|---|
| Channel gain | 0…400 | `raw ≥ 80`: `(raw − 280) / 10` dB. `raw < 80`: `−60 + raw / 2` dB. 280 = 0 dB, 400 = +12 dB, 0 = −60 dB | [code][live] |
| Matrix mix gain | 0…280 | Same scale as channel gain, capped at 0 dB | [code] |
| PEQ and GEQ gain | 0…240 | `(raw − 120) / 10` dB, so ±12 dB in 0.1 dB steps | [code][live] |
| Gate threshold | 0…180 | `(raw − 180) / 2` dB, so −90…0 dB | [code][live] |
| Compressor/limiter threshold | 0…220 | `(raw − 180) / 2` dB, so −90…+20 dB | [code][live] |
| Knee | 0…12 | dB | [code] |

### 4.3 GEQ band centres (31 bands, ISO 1/3 octave) [code]

20, 25, 31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800,
1k, 1.25k, 1.6k, 2k, 2.5k, 3.15k, 4k, 5k, 6.3k, 8k, 10k, 12.5k, 16k, 20k Hz.

### 4.4 Q index (0…100) [code]

```
  0: 0.40  0.42  0.45  0.47  0.50  0.53  0.56  0.59  0.63  0.67
 10: 0.71  0.75  0.79  0.84  0.89  0.94  1.00  1.06  1.12  1.19
 20: 1.26  1.30  1.40  1.50  1.60  1.70  1.80  1.90  2.00  2.10
 30: 2.20  2.40  2.50  2.70  2.80  3.00  3.20  3.40  3.60  3.80
 40: 4.00  4.20  4.50  4.80  5.00  5.30  5.70  6.00  6.30  6.70
 50: 7.10  7.60  8.00  8.50  9.00  9.50  10.1  10.7  11.3  12.0
 60: 12.7  13.5  14.0  15.0  16.0  17.0  18.0  19.0  20.0  21.0
 70: 23.0  24.0  25.0  27.0  29.0  30.0  32.0  34.0  36.0  38.0
 80: 40.0  43.0  45.0  48.0  51.0  54.0  57.0  60.0  64.0  68.0
 90: 72.0  76.0  81.0  85.0  91.0  96.0  102   108   114   121
100: 128
```

### 4.5 PEQ filter type [code]

0 Peak · 1 Low Shelf · 2 High Shelf · 3 LP −6 dB · 4 LP −12 dB · 5 HP −6 dB ·
6 HP −12 dB · 7 Allpass 1 · 8 Allpass 2

### 4.6 HPF/LPF slope (0…19) [code]

BW = Butterworth, BL = Bessel, LK = Linkwitz-Riley, number = dB/octave:

```
 0 BW-6   1 BL-6   2 BW-12  3 BL-12  4 LK-12  5 BW-18  6 BL-18  7 BW-24  8 BL-24  9 LK-24
10 BW-30 11 BL-30 12 BW-36 13 BL-36 14 LK-36 15 BW-42 16 BL-42 17 BW-48 18 BL-48 19 LK-48
```

### 4.7 Time constants [code][live]

| Quantity | Raw range | Conversion |
|---|---|---|
| Delay | 0…65280 | samples at 96 kHz: `ms = raw / 96` (max 680 ms), `m = ms · 0.3435`, `ft = m · 3.2808` |
| Gate/comp/limiter attack | 0…998 | `ms = raw + 1` (1–999 ms) |
| Gate hold | 9…998 | `ms = raw + 1` (10–999 ms) |
| Gate/comp/limiter release | 9…2999 | `ms = raw + 1` (10–3000 ms) |

### 4.8 Compressor ratio index (0…15) [code]

`1:1.0, 1:1.1, 1:1.3, 1:1.5, 1:1.7, 1:2.0, 1:2.5, 1:3.0, 1:3.5, 1:4.0, 1:5.0,
1:6.0, 1:8.0, 1:10, 1:20, 1:∞` (the app stores 20.1 for the last entry and shows
ratios as `1:x`).

## 5. Factory default program (captured) [live]

Useful as a test vector (`Default Preset`):

- All gains 280 (0 dB), polarity 0, delay 0, all mutes off.
- GEQ: every band 120 (0 dB).
- Input PEQ bands (gain/freq/Q/type): 120 / {41, 71, 101, 140, 170, 200, 240, 270}
  (≈ 50, 100, 200, 500, 1k, 2k, 5k, 10k Hz) / 35 (Q 3.0) / 0 (Peak).
- Output PEQ freq indices: {31, 63, 95, 127, 158, 190, 222, 253, 285}, same gain/Q/type.
- HPF 0 / LPF 300, slope 0.
- Gate: attack 0, hold 99, release 99, threshold 0.
- Compressor: ratio 0, attack 49, release 499, knee 0, threshold 220.
- Limiter: attack 49, release 499, knee 0, threshold 220.
- Out1 routing mask 1, mix gains 280/280.

Raw pages 0–7, 10 and 15 of that dump are stored in
`tools/re/captures/default-program-partial.json`. A full read over USB
(`tools/re/captures/usb-01-full-read.json`) has all 19 pages, and they equal
`Program::factory_default()` byte for byte.
