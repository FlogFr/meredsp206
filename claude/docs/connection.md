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
