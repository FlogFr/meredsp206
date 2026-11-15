"""Tests for tools/probe/dsp206_dump.py (no network access)."""
import importlib.util
import json
import pathlib
import struct
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("dsp206_dump", ROOT / "tools/probe/dsp206_dump.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)

CAPTURE = json.loads((ROOT / "tools/re/captures/default-program-partial.json").read_text())


class FrameTest(unittest.TestCase):
    def test_reproduces_every_captured_request(self):
        sent = [bytes.fromhex(f["hex"]) for f in CAPTURE["frames"] if f["dir"] == "tx"]
        self.assertGreater(len(sent), 30)
        for raw in sent:
            self.assertEqual(probe.frame(raw[3], raw[5], raw[6:-3]), raw)

    def test_known_frames(self):
        self.assertEqual(probe.frame(1, 0x10).hex(" "), "10 02 00 01 01 10 10 03 11")
        self.assertEqual(probe.frame(1, 0x27, b"\x00").hex(" "), "10 02 00 01 02 27 00 10 03 25")


class SafetyTest(unittest.TestCase):
    def test_refuses_commands_outside_whitelist(self):
        session = object.__new__(probe.Session)  # no socket needed: the check comes first
        for cmd in (0x20, 0x21, 0x34, 0x35, 0x12):
            with self.assertRaises(ValueError):
                session.request(cmd)

    def test_whitelist_is_read_only(self):
        self.assertEqual(probe.READ_ONLY, {0x10, 0x11, 0x13, 0x14, 0x22, 0x27, 0x29, 0x2C, 0x40})


class SummaryTest(unittest.TestCase):
    def test_decode_summary(self):
        program = bytearray(probe.PROGRAM_SIZE)
        program[2:16] = b"Default Preset"
        program[0x10:0x13] = b"InA"
        struct.pack_into("<H", program, 0x10 + 0x84, 280)  # InA 0 dB
        struct.pack_into("<H", program, 0x128 + 0x5C, 0)  # Out1 -60 dB
        struct.pack_into("<H", program, 0x128 + 0x60, 96)  # Out1 delay 1 ms
        struct.pack_into("<H", program, 0x128 + 0x08, 3)  # Out1 routing A+B
        struct.pack_into("<H", program, 0x382, 0x20)  # Out6 muted
        s = probe.decode_summary(bytes(program))
        self.assertEqual(s["program_name"], "Default Preset")
        self.assertEqual(s["output_mute_mask"], 0x20)
        ina, out1 = s["channels"][0], s["channels"][2]
        self.assertEqual((ina["ch"], ina["name"], ina["gain_db"]), ("InA", "InA", 0.0))
        self.assertEqual((out1["ch"], out1["gain_db"], out1["delay_ms"], out1["routing_mask"]), ("Out1", -60.0, 1.0, 3))
        self.assertEqual(len(s["channels"]), 8)


if __name__ == "__main__":
    unittest.main()
