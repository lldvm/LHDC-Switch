#!/usr/bin/env python3
"""Check app/lang/*.json against en.json.

Reports keys missing from or unknown to each language, and strings whose
printf conversions (%s, %.1f, %%, ...) differ from English. Missing keys are
warnings (the app falls back to English); a conversion mismatch is an error,
since the UI's printf (app/src/ui/src/i18n/printf.ts) and the core's
format_printf (app/src/crates/core/src/messages.rs) would fill in the wrong
argument.

Usage: python tools/check_lang.py [lang ...]
Exit status is 1 if any error was found.
"""
# SPDX-License-Identifier: Apache-2.0 OR MIT

import json
import re
import sys
from pathlib import Path

LANG_DIR = Path(__file__).resolve().parent.parent / "app" / "lang"
CONVERSION = re.compile(r"%(?:%|[-+#0]*\d*(?:\.\d+)?(?:ll|l|z|h)?[diuxXfFeEgGcsp])")


def conversions(s):
    return CONVERSION.findall(s)


def load(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def main(argv):
    en = load(LANG_DIR / "en.json")
    codes = argv or sorted(p.stem for p in LANG_DIR.glob("*.json") if p.stem != "en")
    errors = 0
    for code in codes:
        path = LANG_DIR / f"{code}.json"
        try:
            lang = load(path)
        except (OSError, json.JSONDecodeError) as e:
            print(f"{code}: ERROR cannot load: {e}")
            errors += 1
            continue
        missing = [k for k in en if k not in lang]
        unknown = [k for k in lang if k not in en]
        bad = [k for k in en if k in lang and conversions(en[k]) != conversions(lang[k])]
        for k in missing:
            print(f"{code}: missing {k}")
        for k in unknown:
            print(f"{code}: unknown {k}")
        for k in bad:
            print(f"{code}: ERROR format {k}: {conversions(en[k])} != {conversions(lang[k])}")
        errors += len(bad)
        print(f"{code}: {len(lang)} keys, {len(missing)} missing, {len(unknown)} unknown, {len(bad)} format errors")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
