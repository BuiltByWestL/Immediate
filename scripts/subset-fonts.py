"""
Subset Inter (SIL OFL 1.1) into the single WOFF2 file Immediate ships.

Usage:
    python -I scripts/subset-fonts.py <path/to/InterVariable.ttf>

Requires: pip install fonttools brotli
Source:   https://github.com/rsms/inter/releases (Inter 4.1, InterVariable.ttf)

Keeps both variable axes (wght 100-900, opsz 14-32) and every OpenType feature,
so the Helvetica-Neue-like tuning in src/design/tokens.css (cv10, ss03, tnum, ...)
keeps working. Covers Western and Central European Latin (~110 KB). Other scripts
(Greek, Cyrillic, Vietnamese, CJK — e.g. in file names) fall back to the system
font per glyph. Measured options (WOFF2, all features, opsz axis kept):
    Latin + Ext-A           ~109 KB   <- shipped
    + Ext-B + Vietnamese    ~181 KB
    + Greek + Cyrillic      ~234 KB
"""

import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "fonts" / "InterVariable-subset.woff2"

UNICODES = ",".join(
    [
        "U+0000-00FF",  # Basic Latin + Latin-1 Supplement (includes ×, ·, °)
        "U+0100-017F",  # Latin Extended-A (Central European)
        "U+0131,U+0152-0153,U+02C6,U+02DA,U+02DC",  # dotless i, Œ/œ, spacing accents
        "U+2000-206F",  # General Punctuation (–, —, …, ‘’ “”, •, thin spaces)
        "U+20AC,U+2122",  # €, ™
        "U+2190-2193,U+21E7",  # ← ↑ → ↓, ⇧ (Shift)
        "U+2212,U+2248,U+2264-2265",  # − ≈ ≤ ≥
        "U+2303,U+2318,U+2325,U+23CE",  # ⌃ ⌘ ⌥ ⏎ for shortcut hints
        "U+FEFF,U+FFFD",
    ]
)


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    src = Path(sys.argv[1])
    font = TTFont(src, lazy=False)
    options = subset.Options()
    options.layout_features = ["*"]
    options.name_IDs = ["*"]  # keep feature UI names and the license strings
    options.name_languages = ["*"]
    options.notdef_outline = True
    options.hinting = False  # variable web fonts render unhinted in every target webview
    options.desubroutinize = True

    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=subset.parse_unicodes(UNICODES))
    subsetter.subset(font)

    # Options.flavor only affects fontTools' CLI; the flavor must be set on the font itself.
    font.flavor = "woff2"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    font.save(OUT)
    print(f"{OUT.relative_to(ROOT)}  {OUT.stat().st_size / 1024:.1f} KB  (from {src.stat().st_size / 1024:.0f} KB)")


if __name__ == "__main__":
    main()
