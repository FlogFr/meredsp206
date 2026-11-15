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

The editor supports three links. All of them carry the same frames (section 2).
Over USB HID, each frame travels in its own 64-byte report (section 1.2).

| Link | Details |
|---|---|
| Ethernet (LAN) | TCP client to port **9761** [live][code]. Supported by the Linux app. |
| RS-232 / USB-serial | 115200 baud, 8 data bits, no parity, 1 stop bit [code]. Same frames. |
| USB HID | VID `0x0168` / PID `0x0821`, 64-byte reports [live][code]. Supported by the Linux app (section 1.2). |

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

### 1.2 USB HID

The unit's USB port is a full-speed HID device [live]:

- `idVendor 0x0168` ("Musicrown"), `idProduct 0x0821` ("Dsp Process"), no
  serial number, bus powered, 100 mA.
- One interface, class HID, no subclass or protocol. Interrupt endpoints
  `0x81` IN and `0x02` OUT, 64 bytes, 1 ms interval.
- The 27-byte report descriptor declares one vendor collection (usage page 1,
  usage 0) with a 64-byte input report and a 64-byte output report, 8-bit
  fields, **no report ID**.
- Linux binds `hid-generic`, which creates `/dev/hidrawN`. Its `device/uevent`
  holds `HID_ID=0003:00000168:00000821` and `HID_NAME=Musicrown Dsp Process`.
  The node is root-only by default; `data/udev/70-meredsp206.rules` opens it to
  the logged-in user.

How the editor uses it [code]:

- `HidCtlEnumerate` (`0x42284c`) picks the HID device whose vendor and product
  IDs are `0x168` and `0x821`, then logs `Detect HID`.
- **Output.** `0x4216ac` copies the frame to a buffer after a leading report ID
  `0`, and writes `OutputReportByteLength` bytes (65: the ID and the 64-byte
  report). The rest of the buffer is left as is, so the padding carries no
  meaning. The Linux app pads with zeros, which the unit accepts [live].
  `hidraw` takes the same 65 bytes.
- **Input.** `HidCtlDeviceData` (`0x4224cc`) receives the 64-byte report
  without its ID. It copies `report[4] + 8` bytes, the frame at the start of
  the report, and feeds them to the link's frame parser. The rest of the report
  is ignored. A frame is never split across reports, so every frame must fit in
  64 bytes. All of them do: the longest, a program page reply, takes 60.
- The USB link has its own thread (`0x41cd98`) with the same one-slot queue,
  coalescing and retries as the other links. Its reply timeout starts at
  800 ms (`+0x148 = 0x320`). Before sending `0x10`, the USB connect routine
  (`0x423c64`) calls the link setters with 100 and 3, which look like a 100 ms
  timeout and 3 retries [guess]. The rest of the session (section 3) is the
  same as over the network.

What the unit does over USB [live] (captures `usb-01` to `usb-03`, 2026-11-15):

- Every request got its reply, in 2–21 ms (usually 6–11 ms). A full read
  (handshake, 19 pages, 21 preset names, lock state, slot table) took 0.55 s.
  `0x40` polled every 300 ms for 30 s got 100 replies out of 100. None of the
  network silences ([connection.md](connection.md)) happened.
- **The padding of input reports is stale.** After a short reply, the rest of
  the report still holds bytes of the earlier, longer frames the unit sent,
  even from a previous session, `10 03` trailers included. Bytes 60–63, past
  every frame, read `00 49 b4 53`. So a receiver **must** cut each report by
  the frame length, as the editor does, and must not feed whole reports to a
  stream parser.
- Setters work: `0x35` (mute Out6 on, then off) were acknowledged with `0x01`
  in 11–14 ms, and page 17 read `0x382` = `0x0020`, then `0`.

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
- Any complete frame from the device ends the request in flight, whatever its
  command (`0x40ce7c`, called from the frame parser `0x40d040`).
- **Idle keepalive.** The NET link restarts a 2000 ms timer (`+0x200`, handler
  `0x40c428`) on every send. When it fires with nothing queued or in flight, the
  app sends `0x12` and sets the reply timeout to 5000 ms. The timeout stays at
  5000 ms afterwards (nothing sets it back) [code].
- After the `0x10` reply, the app sends `0x13` with a 5000 ms timeout and 1 retry.
  After a valid identity it sets 1000 ms and continues with `0x2c` (`0x4244ec`,
  `0x4245c8`) [code].
- **Short reads.** When a socket read returns 5 bytes or fewer, `ClientSocketRead`
  (`0x45ce78`) re-sends the last frame at once and logs `NET RW` [code]. We never
  received such short chunks in our captures [live].

### 3.3 Generic replies [code]

| Reply `cmd` | Meaning |
|---|---|
| `0x01` | ACK, no payload. Answers setters: `0x35` (mute) got `10 02 01 00 01 01 10 03 00` after 50–66 ms [live]. Also drives the multi-step store/recall/upload state machines [code]. |
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
| `0x12` | End of bulk read; idle keepalive (section 3.2) | — | ACK `0x01` (over USB) | [live][code] |
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
| `0x35` | Mute. The app sends one frame for `ch` and sets the local mute bit of every channel linked to it (`0x4160fc`, frame built by `0x416244`). Out6 mute on/off checked on the unit (front LED and `0x382`) | `[ch, 0/1]` | [live][code] |
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

- Why the DSP stops answering after a burst or a reconnect over the network (see
  [connection.md](connection.md)). It does not happen over USB (section 1.2).
- Exact meaning of the `0x10` reply byte, the `0x2c` payload and the `0x22` table.
- HPF/LPF bypass encoding. Global words `0x394` and `0x396` (see the memory map).
- Write-path commands (`0x1a`, `0x25`, `0x19`, `0x28`, `0x23`, `0x04`) and the
  ACK state machine of store, recall, upload and download.
- Test-tone payload values and the network/ID setup frames.
