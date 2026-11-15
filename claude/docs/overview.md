# Product overview

**meredsp206** is a Linux replacement for *the t.racks DSP 206 Processor Editor*
(Windows only). It controls a t.racks DSP 206 loudspeaker processor over the LAN.

## 1. The device

- 2 inputs (InA, InB), 6 outputs (Out1–Out6), 96 kHz internal sample rate.
- Front-panel MCU reachable through an Ethernet-to-UART module (TCP port 9761),
  RS-232 or USB.
- Holds the current program plus 20 user presets.
- Firmware identifies itself as `DSP206 V0105P` (unit tested 2026-11-15).

## 2. Feature inventory (what "all features" means)

Taken from the original app's forms, menus and string table (see
[original-app.md](original-app.md)). Each feature links to its protocol command
in [protocol.md](protocol.md) and its storage in [memory-map.md](memory-map.md).

### 2.1 Per input (InA, InB)

| Feature | Range | Cmd |
|---|---|---|
| Gain | −60…+12 dB | `0x34` |
| Mute | on/off | `0x35` |
| Polarity | normal/inverse | `0x36` |
| Delay | 0–680 ms (ms / m / ft display) | `0x38`, unit `0x15` |
| Noise gate | threshold −90…0 dB, attack 1–999 ms, hold 10–999 ms, release 10–3000 ms | `0x3e` |
| 31-band graphic EQ | ±12 dB per ISO band, bypass, reset | `0x48`, `0x49` |
| 8-band parametric EQ | 19.7 Hz–20.16 kHz, Q 0.4–128, ±12 dB, 9 filter types, per-band bypass, EQ bypass, reset | `0x33`, `0x3c` |
| High-pass / low-pass | 19.7 Hz–20.16 kHz, 20 slopes (BW/BL/LR −6…−48 dB/oct), bypass | `0x32`, `0x31` |
| Channel name | 8 ASCII characters | `0x3d` |

### 2.2 Per output (Out1–Out6)

| Feature | Range | Cmd |
|---|---|---|
| Matrix routing | InA and/or InB, mix gain −60…0 dB each | `0x3a`, `0x41` |
| Gain, mute, polarity, delay | as inputs | `0x34`, `0x35`, `0x36`, `0x38` |
| High-pass / low-pass | as inputs | `0x32`, `0x31` |
| 9-band parametric EQ | as inputs | `0x33`, `0x3c` |
| RMS compressor | threshold −90…+20 dB, ratio 1:1…1:∞, attack, release, knee 0–12 dB | `0x30` |
| Peak limiter | threshold −90…+20 dB, attack, release, knee | `0x3f` |
| Channel name | 8 ASCII characters | `0x3d` |

### 2.3 Global

- **Link** channels into groups so one edit applies to all members (`0x3b`).
- **Copy** all settings from one channel to another (`0x2a`).
- **Presets**: list (`0x22`, `0x29`), recall (`0x20`), store with a 14-character
  name (`0x26` then `0x21`).
- **Files**: save/open the current program as `.prs`, upload/download the whole unit
  as `.unt`.
- **Factory default** reset (hidden menu in the original).
- **Test tone**: analog input, pink noise, white noise or sine (`0x39`).
- **Lock**: password lock and "PTS" time-limited lock (`0x2c`–`0x2f`).
- **Device ID and IP setup** (frames not decoded yet).
- **Level meters**: inputs and outputs in dBu, plus compressor/limiter activity
  (`0x40`, polled every 300 ms).
- **Offline/demo mode**: edit a program without a device.

## 3. Goals of the Linux app

- Native to Debian stable (GNOME/Wayland first, works on any desktop).
- Light: one small native binary, low idle CPU, no web runtime.
- Clearer than the original: one screen with all channel strips, then a focused
  editor per channel with large EQ curves, keyboard control and undo.
- Safe: never change device parameters without user action. Show the link state
  honestly (see [connection.md](connection.md)).

The planned architecture is in [architecture.md](architecture.md). The desired
look is in `design/*.png`.
