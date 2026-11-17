"""The Kotlin port in android/core must match the Rust protocol crate.

Runs with `make check`, without a JDK. The Kotlin tests check the same thing
the other way round (android/core/src/test/kotlin/dsp206/proto/DocsSyncTest.kt).
"""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
RUST = ROOT / "crates/dsp206-proto/src/command.rs"
KOTLIN = ROOT / "android/core/src/main/kotlin/dsp206/proto/Command.kt"


def codes(text, start, end):
    """Hex codes between the first `start` marker and the next `end`."""
    i = text.index(start)
    body = text[i : text.index(end, i)]
    return [int(c, 16) for c in re.findall(r"0x([0-9A-Fa-f]{2})", body.split("=", 1)[1])]


class AndroidPortTest(unittest.TestCase):
    def setUp(self):
        self.rust = RUST.read_text()
        self.kotlin = KOTLIN.read_text()

    def test_all_codes_match(self):
        self.assertEqual(
            codes(self.rust, "pub const ALL_CODES", "];"),
            codes(self.kotlin, "val ALL: List<Int>", ")"),
        )

    def test_read_only_codes_match(self):
        self.assertEqual(
            sorted(codes(self.rust, "pub const READ_ONLY_CODES", "];")),
            sorted(codes(self.kotlin, "val READ_ONLY: List<Int>", ")")),
        )

    def test_usb_ids_match_the_device_filter(self):
        hid = (ROOT / "crates/dsp206-proto/src/hid.rs").read_text()
        vid = int(re.search(r"VENDOR_ID: u16 = 0x([0-9a-f]+)", hid).group(1), 16)
        pid = int(re.search(r"PRODUCT_ID: u16 = 0x([0-9a-f]+)", hid).group(1), 16)
        filt = (ROOT / "android/app/src/main/res/xml/usb_device_filter.xml").read_text()
        self.assertIn(f'vendor-id="{vid}"', filt)
        self.assertIn(f'product-id="{pid}"', filt)

    def test_translations_have_every_string(self):
        res = ROOT / "android/app/src/main/res"
        names = lambda f: set(re.findall(r'<(?:string|plurals) name="([^"]+)"(?! translatable="false")', f.read_text()))
        self.assertEqual(names(res / "values/strings.xml"), names(res / "values-fr/strings.xml"))


if __name__ == "__main__":
    unittest.main()
