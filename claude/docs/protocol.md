# DSP 206 control protocol

Reverse engineered from *the t.racks DSP 206 Processor Editor V1.05* (Windows,
C++Builder, built 2022-10-11) and checked against a real unit (firmware string
`DSP206 V0105P`). See [reverse-engineering.md](reverse-engineering.md) for how each
fact was obtained and [memory-map.md](memory-map.md) for the parameter layout.

Confidence tags used below:

- **[live]** observed on the real device.
- **[code]** read directly from the original application's machine code.
- **[guess]** inferred from names, call sites or context. Verify before relying on it.

## 1. Transports

The editor supports three links. All of them carry the same frames (section 2),
except USB HID which uses raw 64-byte reports.

| Link | Details |
|---|---|
| Ethernet (LAN) | TCP client to port **9761** [live][code]. This is the link the Linux app targets. |
| RS-232 / USB-serial | 115200 baud, 8 data bits, no parity, 1 stop bit [code]. Same frames. |
| USB HID | 64-byte reports through `TJvHidDeviceController` [code]. Not documented further. |

### 1.1 Ethernet module

The LAN port is an Ethernet-to-UART bridge built on the Microchip TCP/IP stack
(MAC OUI `00:04:A3`, Microchip Technology) [live].

- **Discovery**: send the ASCII string `Discovery: Who is out there?` followed by a
  NUL byte to UDP port **30303** (broadcast or unicast). The module answers from
  port 30303 with its NetBIOS-style host name and part of its MAC address [live]:

  ```
  "EHTERNET_COM   \r\n00-04-A3-C1\r\n1F\r\n"
  ```

  (sic, `EHTERNET`). The reply's source address is the device IP. The original app
  only uses discovery in its hidden "auto" mode. It defaults to a manually entered
  IP, `192.168.1.101`, which is also the module's factory address [code][live].
- **TCP**: port 9761, **one client at a time**. While a session is open, further SYNs
  are not answered [live].
- **Session recycling**: after the client closes the socket (FIN or RST), the module
  accepts a new TCP connection at once, but the DSP behind it often stops answering
  for roughly **45–90 s** [live]. Keep **one long-lived connection** and never
  reconnect in a loop. See [connection.md](connection.md).
- The module advertises a 256-byte TCP receive window [live].

## 2. Frame format

```
+------+------+-----+-----+-----+-----+----------------+------+------+-----+
| 0x10 | 0x02 | src | dst | len | cmd | data[len - 1]  | 0x10 | 0x03 | chk |
+------+------+-----+-----+-----+-----+----------------+------+------+-----+
  DLE    STX                                             DLE    ETX
```

| Field | Meaning |
|---|---|
| `src` | Sender address. PC → DSP: `0x00`. DSP → PC: the device ID [live][code]. |
| `dst` | Receiver address. PC → DSP: device ID (1–254, factory default **1**). DSP → PC: `0x00` [live][code]. |
| `len` | `1 + len(data)`. It counts the `cmd` byte [live][code]. |
| `cmd` | Command code (section 4). |
| `data` | `len - 1` payload bytes. Multi-byte values are **little-endian u16** [live][code]. |
| `chk` | XOR of every preceding byte of the frame, from the first `0x10` through `0x03` [live][code]. |

- Total frame size = `len + 8` bytes.
- **No DLE stuffing.** Payload bytes equal to `0x10` are sent as is, so a receiver
  must cut frames by length, not by scanning for `10 03` [code][live].
- Receiver algorithm (as in the original app): wait for `10 02`, wait until
  `buf[4] + 8` bytes are available, check `10 03` at offsets `-3..-2` and the XOR,
  then dispatch on `buf[5]`. Frames whose `src` differs from the selected device
  ID are ignored [code].

### 2.1 Captured examples [live]

```
PC  → DSP  10 02 00 01 01 10 10 03 11                                   connect
DSP → PC   10 02 01 00 02 10 19 10 03 0b                                connect reply, data = 19
PC  → DSP  10 02 00 01 01 13 10 03 12                                   identify
DSP → PC   10 02 01 00 0e 13 44 53 50 32 30 36 20 56 30 31 30 35 50 10 03 4c
                                                                        "DSP206 V0105P"
PC  → DSP  10 02 00 01 02 27 00 10 03 25                                read program page 0
DSP → PC   10 02 01 00 34 24 00 ff 00 44 65 66 61 ... 10 03 bf         page 0 (50 data bytes)
```

## 3. Session flow

### 3.1 Connect sequence (original app)

The original app keeps **exactly one request in flight**. It sends the next frame
only after the matching reply arrives or a timeout expires [code].

1. `0x10` connect → reply `0x10 [status]`.
2. `0x13` identify → reply `0x13 "DSP206 V0105P"`. The app requires the text to start
   with `DSP206` (else *Wrong device*) and with `DSP206 V010` (else *Wrong device
   version*). On mismatch it sends `0x11` and gives up [code].
3. `0x2c` lock/PTS status → if locked, a password dialog is shown [code].
4. `0x22` preset slot table [code][live].
5. `0x14` current preset number [code][live].
6. `0x29 [n]` for n = 0…20: preset names [code][live].
7. `0x27 [p]` for p = 0…18 → `0x24 [p, 50 bytes]`: the current program, 928 bytes
   (see [memory-map.md](memory-map.md)) [code][live].
8. `0x12` once all pages are read [code].
9. Steady state: `0x40` level-meter poll every **300 ms** [code].

### 3.2 Timeouts, retries, coalescing [code]

- Default reply timeout: **800 ms** (some long operations use 5 s). Each request is
  retried a configurable number of times before the app reports *Link Broken*.
- While a request is outstanding, new requests are queued. When a queued request
  has the same command byte as the newest queued one, it **replaces** it. This keeps
  fast fader moves from flooding the device. The Linux app should do the same:
  coalesce by `(cmd, channel[, band])`.

### 3.3 Generic replies [code]

| Reply `cmd` | Meaning |
|---|---|
| `0x01` | ACK. Drives the multi-step store/recall/upload state machines. |
| `0x02` | Seen once after a request the device was not ready for [live]. Probably NAK/busy [guess]. |

## 4. Command reference

`ch` is a channel index: **0 = InA, 1 = InB, 2…7 = Out1…Out6** [code]. All
setters exist in the original app as "send one parameter now" functions that also
update the in-memory program (same offsets as [memory-map.md](memory-map.md)).
The device mirrors those fields in its current program.

### 4.1 Session and information

| Cmd | Name | Request data | Reply | Tag |
|---|---|---|---|---|
| `0x10` | Connect | — | `0x10 [status]` (observed `0x19`) | [live] |
| `0x11` | Disconnect | — | none | [live] |
| `0x12` | End of bulk read / refresh | — | ACK? | [code] |
| `0x13` | Identify | — | `0x13` ASCII, e.g. `DSP206 V0105P` | [live] |
| `0x14` | Get current preset number | — | `0x14 [n]` | [live] |
| `0x2c` | Get lock / PTS state | — | `0x2c [6 bytes]`, observed `00 27 0f 00 00 00`; byte 0 ≠ 0 means locked | [live][guess] |
| `0x40` | Get level meters | — | `0x40 [38 bytes]` (section 4.6) | [live][code] |

### 4.2 Presets and bulk transfer

The device keeps the **current program** plus **20 user presets** (U01–U20). Name
slot 0 seems to be the factory/current one, giving 21 names [code][live].

| Cmd | Name | Request data | Reply | Tag |
|---|---|---|---|---|
| `0x22` | Get preset slot table | — | `0x22 [20 bytes]`. `0xFF` = slot flag set (empty?) | [live][guess] |
| `0x29` | Get preset name | `[n]` (0–20) | `0x29 [n, 14 ASCII bytes]` | [live] |
| `0x27` | Read current-program page | `[p]` (0–18) | `0x24 [p, 50 bytes]` | [live] |
| `0x28` | Read stored-preset page (*Upload* menu) | `[p]` | `0x28 [p, …]` | [code] |
| `0x19` | Start full-unit upload | `[u16, u8]` | `0x19 …` | [code] |
| `0x1a` | Write program page (*Download*) | `[u16, u8, data…]` | ACK | [code] |
| `0x25` | Bulk transfer step (unknown) | `[u8, u8, data…]` | `0x25 …` | [code] |
| `0x26` | Set preset name before storing | `[14 ASCII bytes]` | ACK | [code][guess] |
| `0x20` | Store current program into preset `n` | `[n]` | ACK | [code][guess] |
| `0x21` | Recall preset `n` | `[n]` | ACK, then re-read pages | [code][guess] |
| `0x23` | Unknown (used by *Upload* and lock flows) | — | ? | [code] |
| `0x04` | Unknown, no data | — | ? | [code] |

> `0x20`, `0x21`, `0x1a`, `0x26` and every command in 4.3 **change the sound**. Do
> not send them to the device without the user's explicit agreement. See
> `claude/rules/device-safety.md`.

### 4.3 Parameter setters

Values use the encodings of [memory-map.md §4](memory-map.md#4-value-encodings).

| Cmd | Parameter | Request data | Tag |
|---|---|---|---|
| `0x34` | Channel gain | `[ch, gain u16]` | [code] |
| `0x35` | Mute | `[ch, 0/1]` | [code] |
| `0x36` | Polarity (phase invert) | `[ch, 0/1]` | [code] |
| `0x38` | Delay | `[ch, samples u16]` | [code] |
| `0x15` | Delay display unit (0 = ms, 1 = m, 2 = ft) | `[unit]` | [code][guess] |
| `0x33` | PEQ band | `[ch, band, gain u16, freq u16, q, type, bypass]` | [code] |
| `0x3c` | PEQ bypass, whole channel | `[ch, 0/1]` | [code] |
| `0x48` | GEQ band (inputs) | `[ch, band 0–30, gain u16]` | [code] |
| `0x49` | GEQ bypass (inputs) | `[ch, 0/1]` | [code] |
| `0x32` | High-pass (low cut) | `[ch, freq u16, slope]` | [code] |
| `0x31` | Low-pass (high cut) | `[ch, freq u16, slope]` | [code] |
| `0x3e` | Gate (inputs) | `[ch, attack u16, hold u16, release u16, threshold u16]` | [code] |
| `0x30` | Compressor (outputs) | `[ch, ratio u16, attack u16, release u16, knee u16, threshold u16]` | [code] |
| `0x3f` | Limiter (outputs) | `[ch, attack u16, release u16, knee u16, threshold u16]` | [code] |
| `0x3a` | Matrix routing | `[ch(out), input mask]`. Bit 0 = InA, bit 1 = InB | [code] |
| `0x41` | Matrix mix gain | `[ch(out), input 0/1, gain u16]` | [code] |
| `0x3b` | Link group | `[ch, link mask]` | [code] |
| `0x2a` | Copy channel | `[src ch, dst ch]` | [code][guess] |
| `0x3d` | Channel name | `[ch, 8 ASCII bytes]` | [code] |
| `0x39` | Test-tone generator | `[source, value]`. Sources: analog in, pink noise, white noise, sine | [code][guess] |
| `0x2d`, `0x2e`, `0x2f` | Lock / PTS password operations | variable | [code] |

The field order of the dynamics commands (`0x30`, `0x3e`, `0x3f`) follows the
memory map. The app builds them straight from the stored words, so the order is
very likely right, but no frame has been captured yet.

How HPF/LPF **bypass** is encoded is not known yet. The bypass buttons call the
same `0x32`/`0x31` senders, so bypass probably travels in the slope or frequency
field [code][guess].

### 4.4 Device ID and network settings

The *Network/ID setup* dialog edits the device ID (1–254) and the module IP. The
frames it sends are not decoded yet [code].

### 4.5 RS-232 "extend remote control" subset

The app also embeds a help page for a separate, simpler protocol meant for
third-party controllers [code]. Frames are `7B 7D id cmd d1 d2 d3 7D 7B` with no
checksum:

| Cmd | Action | Payload |
|---|---|---|
| `0x41` | Gain step | `in/out (0/1), channel 0–7, +/- (0/1)` |
| `0x42` | Mute | `in/out, channel, 0/1` |
| `0x43` | Load preset | `factory/user (0/1), preset 0–30, 0x30` |
| `0x44` | Input volume | `channel, hi, lo` |
| `0x45` | Output volume | `channel, hi, lo` |
| `0x48` | Get gain | `in/out, channel, 0` → 0–400 (same gain scale) |
| `0x49` | Get mute | `in/out, channel, 0` → 0/1 |
| `0x4A` | Get current preset | `0, 0` → `0x00` = F00, `1..30` = U01..U30 |

It is unclear whether the LAN port accepts this subset. It is documented for
completeness only.

### 4.6 Level meters (`0x40` reply) [live][code]

Payload: 38 bytes = 10 groups of 3 bytes, then 8 status bytes.

- Group `i` (bytes `3i .. 3i+2`) is a little-endian IEEE-754 float32 assembled as
  `[0x00, d[3i+2], d[3i], d[3i+1]]`. It is a linear amplitude.
- Level in dBu = `20·log10(x) + 18`. The app floors 0 to a tiny value.
- Group order: 0 = InA, 1 = InB, 2–3 = unused (InC/InD on bigger models), 4–9 = Out1–Out6.
- Bytes 30–31: u16 used for compressor/limiter activity LEDs [code][guess].
- Meter LED thresholds in the original UI: −50, −40, −30, −20, −5, 0, +5, +12, +15 dBu.
- Measured idle noise floor: about −88 dBu.

## 5. Open questions

- Why the DSP stops answering after a burst or a reconnect (see
  [connection.md](connection.md)). A power cycle of the DSP should be the next test.
- Exact meaning of the `0x10` reply byte, the `0x2c` payload and the `0x22` table.
- HPF/LPF bypass encoding. Global words `0x394` and `0x396` (see the memory map).
- Write-path commands (`0x1a`, `0x25`, `0x19`, `0x28`, `0x23`, `0x04`) and the
  ACK state machine of store, recall, upload and download.
- Test-tone payload values and the network/ID setup frames.
