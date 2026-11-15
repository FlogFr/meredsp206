"""Documentation consistency checks (claude/rules/documentation.md)."""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SKIP = {".git", "target"}


def markdown_files():
    return [p for p in ROOT.rglob("*.md") if not SKIP & set(p.relative_to(ROOT).parts)]


class DocsTest(unittest.TestCase):
    def test_relative_links_resolve(self):
        broken = []
        for md in markdown_files():
            for target in re.findall(r"\]\(([^)#\s]+)(?:#[^)]*)?\)", md.read_text()):
                if "://" in target or target.startswith("mailto:"):
                    continue
                if not (md.parent / target).exists():
                    broken.append(f"{md.relative_to(ROOT)} -> {target}")
        self.assertEqual(broken, [])

    def test_claude_md_indexes_every_doc_and_rule(self):
        index = (ROOT / "CLAUDE.md").read_text()
        for folder in ("claude/docs", "claude/rules"):
            for doc in sorted((ROOT / folder).glob("*.md")):
                rel = doc.relative_to(ROOT).as_posix()
                self.assertIn(f"({rel})", index, f"{rel} is not linked from CLAUDE.md")

    def test_design_png_has_html_source(self):
        for png in (ROOT / "design").glob("*.png"):
            self.assertTrue((ROOT / "design/src" / (png.stem + ".html")).exists(), png.name)

    def test_docs_are_written_in_english(self):
        # Cheap guard against French slipping into project docs: count common
        # French function words in prose, ignoring code spans and blocks.
        french = re.compile(r"\b(le|la|les|des|une|est|pour|avec|dans|sont)\b", re.IGNORECASE)
        for md in markdown_files():
            prose = re.sub(r"```.*?```|`[^`]*`", "", md.read_text(), flags=re.S)
            hits = [w for w in french.findall(prose) if w.lower() not in {"la"}]
            self.assertLess(len(hits), 3, f"{md.relative_to(ROOT)} looks French: {hits[:5]}")


if __name__ == "__main__":
    unittest.main()
