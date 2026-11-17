"""App icons and desktop integration (tools/icon/make_icons.py, data/)."""
import configparser
import importlib.util
import pathlib
import re
import struct
import unittest
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[2]


def load_generator():
    spec = importlib.util.spec_from_file_location("make_icons", ROOT / "tools/icon/make_icons.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ICONS = load_generator()
APP_ID = re.search(r'pub const APP_ID: &str = "([^"]+)"', (ROOT / "crates/dsp206-gtk/src/app.rs").read_text()).group(1)


class GeneratedIconsTest(unittest.TestCase):
    def test_committed_files_match_the_generator(self):
        for rel, text in ICONS.build().items():
            self.assertEqual((ROOT / rel).read_text(), text, f"{rel} is stale: run make icons")

    def test_every_file_is_well_formed_xml(self):
        for rel in ICONS.build():
            ET.parse(ROOT / rel)

    def test_linux_icon_is_named_after_the_app_id(self):
        self.assertEqual(ICONS.APP_ID, APP_ID)
        svg = ET.parse(ROOT / f"data/icons/hicolor/scalable/apps/{APP_ID}.svg").getroot()
        self.assertEqual(svg.get("viewBox"), "0 0 128 128")

    def test_android_adaptive_icon_uses_the_generated_layers(self):
        res = ROOT / "android/app/src/main/res"
        manifest = (ROOT / "android/app/src/main/AndroidManifest.xml").read_text()
        self.assertIn('android:icon="@mipmap/ic_launcher"', manifest)
        self.assertIn('android:roundIcon="@mipmap/ic_launcher_round"', manifest)
        for name in ("ic_launcher", "ic_launcher_round"):
            text = (res / f"mipmap-anydpi-v26/{name}.xml").read_text()
            for layer in re.findall(r'@drawable/(\w+)', text):
                self.assertTrue((res / f"drawable/{layer}.xml").exists(), layer)
            self.assertIn("ic_launcher_monochrome", text)

    def test_android_content_stays_in_the_safe_zone(self):
        # Launchers may cut everything outside a 66 dp circle centred on the
        # 108 dp layer. The motif's content box (x 6..94, y 10..94) must fit.
        s = ICONS.ANDROID_SCALE
        tx, ty = ICONS.transform_for(s, 54, 54)
        corners = [(6, 10), (94, 10), (6, 94), (94, 94)]
        radius = max(((tx + x * s - 54) ** 2 + (ty + y * s - 54) ** 2) ** 0.5 for x, y in corners)
        # The rounded corners of the screen and bars buy a few dp more.
        self.assertLess(radius, 33 + 2.5)

    def test_store_icon_is_a_512_png(self):
        data = (ROOT / "android/store/icon-512.png").read_bytes()
        self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(struct.unpack(">II", data[16:24]), (512, 512))
        self.assertLess(len(data), 1024 * 1024)  # Play Console limit

    def test_no_vendor_names_in_the_artwork(self):
        for rel in ICONS.build():
            text = (ROOT / rel).read_text().lower()
            for name in ("t.racks", "thomann", "musicrown"):
                self.assertNotIn(name, text, rel)


class DesktopEntryTest(unittest.TestCase):
    def setUp(self):
        self.path = ROOT / f"data/{APP_ID}.desktop"
        parser = configparser.ConfigParser(interpolation=None)
        parser.optionxform = str  # keys are case sensitive
        parser.read(self.path)
        self.entry = parser["Desktop Entry"]

    def test_entry_matches_the_app(self):
        gtk_bin = re.search(r'\[\[bin\]\]\s*name = "([^"]+)"', (ROOT / "crates/dsp206-gtk/Cargo.toml").read_text()).group(1)
        self.assertEqual(self.entry["Type"], "Application")
        self.assertEqual(self.entry["Exec"].split()[0], gtk_bin)
        # GNOME matches the window's app ID with the desktop file name.
        self.assertEqual(self.entry["Icon"], APP_ID)
        for key in ("Name", "Comment", "Categories"):
            self.assertTrue(self.entry[key])
        self.assertTrue(self.entry["Categories"].endswith(";"))

    def test_make_install_ships_the_entry_and_the_icon(self):
        makefile = (ROOT / "Makefile").read_text()
        self.assertIn(f"APP_ID = {APP_ID}", makefile)
        self.assertIn("data/$(APP_ID).desktop", makefile)
        self.assertIn("data/icons/hicolor/scalable/apps/$(APP_ID).svg", makefile)


if __name__ == "__main__":
    unittest.main()
