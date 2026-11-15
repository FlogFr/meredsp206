#!/usr/bin/env python3
"""Read-only dump of a t.racks DSP 206 over the LAN.

Opens one TCP session, reads identity, lock state, preset names, the current
program (19 pages) and a few meter frames, then writes everything to JSON.
Only read-only commands can be sent (see claude/rules/device-safety.md).
Wait at least 60 s between two runs (see claude/docs/connection.md).
"""
import argparse
import json
import socket
import struct
import sys
import time

READ_ONLY = {0x10, 0x11, 0x13, 0x14, 0x22, 0x27, 0x29, 0x2C, 0x40}
REPLY = {0x27: 0x24}  # request -> reply command when they differ
PAGES, PAGE_SIZE, PROGRAM_SIZE = 19, 50, 928


def frame(dev, cmd, data=b""):
    body = bytes([0x10, 0x02, 0x00, dev, len(data) + 1, cmd]) + data + b"\x10\x03"
    chk = 0
    for b in body:
        chk ^= b
    return body + bytes([chk])


class Session:
    def __init__(self, host, port, dev, timeout, retries):
        self.dev, self.timeout, self.retries = dev, timeout, retries
        self.sock = socket.create_connection((host, port), timeout=3)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.sock.settimeout(0.02)
        self.buf = b""
        self.t0 = time.monotonic()
        self.log = []

    def _frames(self):
        try:
            data = self.sock.recv(4096)
            if data:
                self.buf += data
        except socket.timeout:
            pass
        while True:
            i = self.buf.find(b"\x10\x02")
            if i < 0:
                self.buf = self.buf[-1:]
                return
            self.buf = self.buf[i:]
            if len(self.buf) < 9 or len(self.buf) < self.buf[4] + 8:
                return
            f, self.buf = self.buf[: self.buf[4] + 8], self.buf[self.buf[4] + 8 :]
            chk = 0
            for b in f[:-1]:
                chk ^= b
            ok = f[-3:-1] == b"\x10\x03" and chk == f[-1]
            self.log.append({"t": round(time.monotonic() - self.t0, 3), "dir": "rx", "hex": f.hex(), "ok": ok})
            if ok:
                yield f

    def request(self, cmd, data=b"", expect=True):
        if cmd not in READ_ONLY:
            raise ValueError("command 0x%02x is not in the read-only whitelist" % cmd)
        want = REPLY.get(cmd, cmd)
        for attempt in range(self.retries + 1):
            f = frame(self.dev, cmd, data)
            self.sock.sendall(f)
            self.log.append({"t": round(time.monotonic() - self.t0, 3), "dir": "tx", "hex": f.hex(), "try": attempt})
            if not expect:
                return None
            end = time.monotonic() + self.timeout
            while time.monotonic() < end:
                for f in self._frames():
                    if f[5] == want:
                        return f[6:-3]
        return None

    def close(self):
        self.sock.close()


def decode_summary(program):
    def u16(o):
        return struct.unpack_from("<H", program, o)[0]

    def name(o, n):
        return program[o : o + n].split(b"\0")[0].decode("ascii", "replace")

    def gain_db(raw):
        return (raw - 280) / 10 if raw >= 80 else -60 + raw / 2

    chans = []
    for k in range(2):
        b = 0x10 + k * 0x8C
        chans.append({"ch": "In" + "AB"[k], "name": name(b, 8), "gain_db": gain_db(u16(b + 0x84)),
                      "delay_ms": round(u16(b + 0x88) / 96, 2), "polarity": u16(b + 0x86)})
    for k in range(6):
        b = 0x128 + k * 0x64
        chans.append({"ch": "Out%d" % (k + 1), "name": name(b, 8), "gain_db": gain_db(u16(b + 0x5C)),
                      "delay_ms": round(u16(b + 0x60) / 96, 2), "polarity": u16(b + 0x5E),
                      "routing_mask": u16(b + 0x08)})
    return {"program_name": name(2, 14), "input_mute_mask": u16(0x380),
            "output_mute_mask": u16(0x382), "channels": chans}


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--host", default="192.168.1.101")
    ap.add_argument("--port", type=int, default=9761)
    ap.add_argument("--id", type=int, default=1, help="device ID (1-254)")
    ap.add_argument("--timeout", type=float, default=0.8)
    ap.add_argument("--retries", type=int, default=3)
    ap.add_argument("--meters", type=int, default=3, help="number of 0x40 meter reads")
    ap.add_argument("-o", "--out", default="dsp206-dump.json")
    args = ap.parse_args()

    s = Session(args.host, args.port, args.id, args.timeout, args.retries)
    result = {"host": args.host, "id": args.id}
    try:
        hello = s.request(0x10)
        if hello is None:
            print("no reply to connect (0x10); wait 60 s and retry", file=sys.stderr)
            return 2
        result["connect"] = hello.hex()
        ident = s.request(0x13)
        result["identity"] = ident.decode("ascii", "replace") if ident else None
        lock = s.request(0x2C)
        result["lock_state"] = lock.hex() if lock else None
        slots = s.request(0x22)
        result["preset_slots"] = slots.hex() if slots else None
        cur = s.request(0x14)
        result["current_preset"] = cur[0] if cur else None
        # 0x29 [i] names U(i+1); i = 20 is the current program (protocol.md 4.2).
        names = {}
        for n in range(21):
            r = s.request(0x29, bytes([n]))
            key = f"U{n + 1:02d}" if n < 20 else "current"
            names[key] = r[1:].split(b"\0")[0].decode("ascii", "replace") if r else None
        result["preset_names"] = names
        program = bytearray(PAGES * PAGE_SIZE)
        missing = []
        for p in range(PAGES):
            r = s.request(0x27, bytes([p]))
            if r and r[0] == p:
                program[p * PAGE_SIZE : p * PAGE_SIZE + len(r) - 1] = r[1:]
            else:
                missing.append(p)
            print("page %2d %s" % (p, "missing" if p in missing else "ok"), file=sys.stderr)
        result["program_hex"] = bytes(program[:PROGRAM_SIZE]).hex()
        result["missing_pages"] = missing
        if not missing:
            result["summary"] = decode_summary(bytes(program[:PROGRAM_SIZE]))
        result["meters"] = []
        for _ in range(args.meters):
            r = s.request(0x40)
            result["meters"].append(r.hex() if r else None)
            time.sleep(0.3)
        s.request(0x11, expect=False)
    finally:
        result["frames"] = s.log
        s.close()
        with open(args.out, "w") as fh:
            json.dump(result, fh, indent=1)
    answered = sum(1 for e in s.log if e["dir"] == "rx")
    sent = sum(1 for e in s.log if e["dir"] == "tx")
    print("wrote %s (%d frames sent, %d received, %d pages missing)" % (args.out, sent, answered, len(result.get("missing_pages", []))), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
