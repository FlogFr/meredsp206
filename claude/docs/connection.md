# Connection behaviour and quirks

What we observed when talking to a real DSP 206 (Ethernet module MAC
`00:04:a3:c1:ab:85`, IP `192.168.1.101`, firmware `DSP206 V0105P`) on
2026-11-15, and what the Linux client must do about it.

## 1. Observations

1. **Single TCP client.** While a TCP session to port 9761 is open, a second
   `connect()` times out (the SYN is never answered).
2. **The first session after a long idle period works well.** In the first test,
   `0x10`, `0x13` and `0x11` all got correct replies. In a later session about 35
   requests in a row were answered at about 50 ms each (connect, identify, lock
   state, preset table, 21 preset names, 8 program pages).
3. **Then replies stop.** After the burst (about 1.8 s in), most requests got no
   reply at all. TCP itself was healthy: no retransmissions, every segment
   acknowledged, 256-byte window. The bytes reach the module, but the DSP's
   answer never comes back. From then on, roughly one request every 4–5 s was
   answered.
4. **Reconnecting does not reset it.** New sessions opened a few seconds after
   closing the previous one (with FIN or with RST) usually got only the first
   reply, or none. After about 45–90 s of silence, the next session again got at
   least its first reply.
5. Sending `0x11` (disconnect) before closing did not visibly change the outcome.
6. A `0x02` reply (no payload) appeared once, right after a request sent while
   the device was apparently still busy.

These observations come from a short series of probes. They are **not explained
yet**. Possible causes, from most to least likely:

- The DSP or the module was left in a bad state by the earlier rapid
  connect/disconnect cycles. **Next test:** power-cycle the DSP, then run one
  calm session (`tools/probe/dsp206_dump.py`).
- Flow control between the Ethernet module and the DSP's UART loses frames once
  the module's small buffers fill up.
- The firmware expects the exact pacing of the original app: one request in
  flight, 800 ms timeout, then a 300 ms `0x40` poll.

### 1.1 USB

Everything above concerns the Ethernet module. Over USB (protocol.md §1.2) the
PC talks to the DSP without that bridge, and the Linux app holds the `hidraw`
node for the whole session. **Over USB the DSP never went silent** (sessions
U1–U4 in section 3.2): every request was answered within about 20 ms. This
points at the Ethernet module, or the DSP's UART link to it, as the cause of
the network silences [guess]. Prefer USB when the unit is within cable reach.

## 2. Rules for the Linux client

- Open **one** TCP connection per device and keep it for the whole app session.
  Never open a second one, not even for discovery checks.
- Exactly **one request in flight**. Wait for the reply whose `cmd` matches the
  expected reply code (for example `0x27` → `0x24`), or for an 800 ms timeout.
- On timeout, retry up to 3 times with the same frame, then report the link as
  degraded in the UI. Do not tear down the socket.
- Coalesce queued setters by `(cmd, ch[, band])` so a dragged fader sends only the
  latest value.
- When the socket must be closed, close it once and wait at least **60 s** before
  reconnecting automatically. Show a countdown instead of hammering the device.
- Poll levels with `0x40` no faster than every 300 ms, and only while a view that
  shows meters is visible.
- Use UDP discovery (port 30303) to find devices. Take the IP from the reply's
  source address, not from its text.

## 3. Test log

All sessions ran on 2026-11-15, in this order, with gaps of a few seconds to a few
minutes between them.

| # | Session | Result |
|---|---|---|
| 1 | `10 13 11` | all answered |
| 2 | `10 13 2c 22 14 29×3 11` | `10` ok, `13` none, `22` → `0x02`, rest none |
| 3–5 | several short sessions | first reply at most |
| 6 | full dump with 1 s timeout | 35 replies in a row, then 1 reply every ~4 s |
| 7 | same with 0.6 s timeout and 4 retries | only `10`, `13`, `14` |
| 8 | gentle pacing (150 ms gaps, 2 s timeout) | `10`, `13`, `2c`, then nothing |
| 9 | `0x13` every 2.5 s for 30 s | every other request answered (~5 s period) |
| 10 | `0x40` every 300 ms after connect | only the connect reply |
| 11 | three parallel TCP sessions | the second `connect()` timed out |
| 12 | `tools/probe/dsp206_dump.py`, >10 min after #11 | 18 replies in 1.0 s (connect, identify, lock, slots, preset, names 0–12), then silence. One more reply at 6.7 s, none for the next 25 s |

Runs 6 and 12 both stop after a clean burst, at 1.8 s and 1.0 s after the connect
and after about 335 and 175 bytes sent. Neither the elapsed time nor the byte
count gives a consistent threshold.

### 3.1 Packet captures (afternoon of 2026-11-15)

Each session below started at least 85 s after the previous one ended, with a
packet capture (`dumpcap`) running. Raw logs: `tools/re/captures/link-*.json`.

| # | Session | Result |
|---|---|---|
| 13 | original-app sequence (`10 13 2c 22 14 29×21 27×19 12`), back to back | 17 replies in 0.92 s, one more at 1.77 s, then one `0x02` at 17.9 s and nothing else |
| 14 | `10 13`, then `0x40` every 300 ms for 25 s | 8 replies until 1.70 s, then 6 isolated replies in 23 s |
| 15 | TCP connect, **3 s idle**, then `10 13` and `0x40` every 300 ms | **no reply at all** |
| 16 | same as #14, as a control for #15 | 7 replies until 1.40 s, then silence |

What the captures show [live]:

- **TCP stays healthy.** The module acknowledges every request 7–12 ms after it
  arrives and its receive window stays at 256 bytes, in the good phase and in the
  silent phase alike. The bytes reach the module and leave its TCP buffer.
- **Replies are all or nothing.** Every reply that arrives is a complete frame with
  a valid checksum. No stray or partial bytes were ever received.
- **The responsive window is tied to the TCP connection, not to the requests.**
  It lasts 0.9–1.8 s after the TCP handshake, whatever the request rate. Waiting
  3 s before the first request (#15) loses it completely, `0x10` included.
- In the silent phase, a request sent 0.3 s after an answered one was never
  answered. Requests 0.8 s apart were answered about one time in four.
- The occasional `0x02` reply suggests that the DSP sometimes receives a damaged
  frame [guess].
- The module ARPs for the gateway (`192.168.1.1`) twice right after each TCP
  connect. The replies still go straight to the PC's MAC address.

Later sessions, still at least 90 s apart (captures `link-17` to `link-23`):

| # | Session | Result |
|---|---|---|
| 17 | as #13, right after a **power cycle** of the DSP | 37 replies until 1.96 s, then 2 isolated replies. The power cycle changes nothing |
| 18 | `10`, mute Out6 on, mute Out6 off, read page 17 (user-approved) | `0x10` and the first mute answered (`0x01` ACK after 66 ms), then silence from 0.15 s. The Out6 MUTE LED lit |
| 19 | `10`, read page 17, mute Out6 off, read page 17 | all answered: `0x382` read `0x20`, then `0` after the unmute. The LED went off |
| 20 | as #13 with a **200 ms pause** after each reply | answered in bursts until 10.85 s (21 replies), with gaps of 1.1, 1.9 and 3.5 s, then one `0x02` at 21.6 s and nothing else |
| 21 | repeat of #15 (3 s idle after the TCP connect) | no reply at all, again |
| 22 | `10`, then 60 s idle **while connected**, then requests | only the `0x10` reply, then one `0x02` 125 s in. Staying connected and quiet does not bring the DSP back |
| 23 | six **short sessions** (`10 13 40 11`, close) 2 s apart | the first got its 3 replies in 0.19 s, the next five nothing (one `0x02`) |

So far:

- A session gets a working window only when it opens at least ~90 s after the
  previous one, and only if it talks right away. Neither a power cycle, nor
  pacing, nor reconnecting, nor staying quiet restores the link.
- Inside the window, setters work and are acknowledged.
- Long silences often end with a `0x02` reply. This fits a DSP that lost the
  start of a frame and waits for the rest of a long phantom frame, swallowing
  the next requests [guess].

### 3.2 USB sessions (evening of 2026-11-15)

Same unit, plugged into the PC over USB (`/dev/hidraw5`), sessions about 60 s
apart, the network cable unused. Captures `tools/re/captures/usb-*.json`.

| # | Session | Result |
|---|---|---|
| U1 | `dsp206 --usb info`: `10 13`, `27`×19, `12`, `14`, `29`×21, `2c`, `22` | all 45 answered in 0.55 s, 6–21 ms each. The 19 pages equal the factory default program |
| U2 | `dsp206 --usb meters 100`: `10`, then `40` every 300 ms for 30 s | 101 replies to 101 requests, about 10 ms each |
| U3 | `10`, page 17, mute Out6 on, 1 s, page 17, mute Out6 off, page 17, `11` (user-approved) | all answered; `0x382` read `0`, `0x20`, `0`. `0x11` gets no reply, as over the network |
| U4 | GTK app, `meredsp206 --usb`: full read, then meters every 300 ms for about 30 s | identity, preset and program shown; closed by killing the process |
| U5 | Read-only after the user's "store U01" with the swapped codes: `10`, `14`, `29` 0–3, `11` | current preset 1 (was 0 in U1), U01 still "Default Preset": the unit had loaded U01 (protocol.md §4.2) |
