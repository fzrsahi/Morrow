#!/usr/bin/env python3
"""#239 AC 4 — extract the Swift string tables into one shared resource.

Reads `macos/Sources/Morrow/L10n.swift`, writes:

  shared/contract/strings/<lang>.json          the source of truth (--extract
                                               creates it from Swift once)
  macos/Sources/Morrow/L10n+Strings.swift      generated; same values, emitted
  shared/desktop/src/lib/strings.generated.ts  generated; read by the desktop

`--check` writes nothing and exits non-zero when any output differs from what the
generator would produce. That is the anti-drift gate: edit the JSON and forget to
regenerate, or edit a generated file by hand, and it fails.

`--extract` is the one-way first step: it read the tables out of `L10n.swift` when
they lived there. The JSON is authoritative now, so extracting again would have
nothing to read.

Parsing Swift literals by hand is deliberate — adding a Swift parser to a bash
script is worse, and the file is one literal per line.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SWIFT = ROOT / "macos/Sources/Morrow/L10n.swift"
JSON_DIR = ROOT / "shared/contract/strings"
SWIFT_OUT = ROOT / "macos/Sources/Morrow/L10n+Strings.swift"
TS_OUT = ROOT / "shared/desktop/src/lib/strings.generated.ts"

# Swift variable → BCP-47-ish tag, in the order `L10n.strings` lists them.
LANGS = [
    ("en", "en"),
    ("zh", "zh"),
    ("zhHant", "zh-Hant"),
    ("de", "de"),
    ("ja", "ja"),
    ("ko", "ko"),
    ("tr", "tr"),
]

ESCAPES = {
    "n": "\n",
    "t": "\t",
    "r": "\r",
    '"': '"',
    "\\": "\\",
    "0": "\0",
}


def unescape(literal: str) -> str:
    out = []
    i = 0
    while i < len(literal):
        ch = literal[i]
        if ch != "\\":
            out.append(ch)
            i += 1
            continue
        nxt = literal[i + 1]
        if nxt == "u" and literal[i + 2] == "{":
            end = literal.index("}", i + 3)
            out.append(chr(int(literal[i + 3 : end], 16)))
            i = end + 1
            continue
        out.append(ESCAPES.get(nxt, nxt))
        i += 2
    return "".join(out)


def escape_swift(text: str) -> str:
    out = []
    for ch in text:
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\r":
            out.append("\\r")
        elif ord(ch) < 0x20:
            out.append("\\u{%X}" % ord(ch))
        else:
            out.append(ch)
    return "".join(out)


def escape_ts(text: str) -> str:
    return json.dumps(text, ensure_ascii=False)


LINE = re.compile(r'^\s*"((?:[^"\\]|\\.)*)"\s*:\s*"((?:[^"\\]|\\.)*)"\s*,\s*$')


def body(text: str, start: int) -> str:
    """The dictionary body starting at `start`, up to its closing `    ]`."""
    end = text.index("\n    ]", start)
    return text[start:end]


def parse_table(text: str, var: str) -> tuple[list[tuple[str, str]], bool]:
    """(pairs, is_override). `de`/`ja`/`ko`/`tr` are `en.merging([...])`."""
    override = re.search(
        rf"static let {var}: \[String: String\] = en\.merging\(\[", text
    )
    if override:
        pairs, _ = scan(text, override.end(), "])")
        return pairs, True
    plain = re.search(rf"static let {var}: \[String: String\] = \[", text)
    if not plain:
        raise SystemExit(f"cannot find the table for {var}")
    pairs, _ = scan(text, plain.end(), "\n    ]")
    return pairs, False


def scan(text: str, start: int, terminator: str) -> tuple[list[tuple[str, str]], int]:
    """Every literal pair until `terminator`; comments are skipped."""
    end = text.index(terminator, start)
    pairs: list[tuple[str, str]] = []
    for line in text[start:end].split("\n"):
        match = LINE.match(line)
        if match:
            pairs.append((unescape(match.group(1)), unescape(match.group(2))))
    return pairs, end


def extract() -> dict[str, dict[str, str]]:
    source = SWIFT.read_text(encoding="utf-8")
    tables: dict[str, dict[str, str]] = {}
    overrides: dict[str, dict[str, str]] = {}
    for var, tag in LANGS:
        pairs, is_override = parse_table(source, var)
        if len(pairs) != len({key for key, _ in pairs}):
            raise SystemExit(f"{var} has a duplicate key")
        if is_override:
            overrides[tag] = dict(pairs)
        else:
            tables[tag] = dict(pairs)
    if "en" not in tables:
        raise SystemExit("the base table `en` is missing")
    # `L10n.subscript` falls back to `en` for a key a language does not carry, so
    # resolving the overrides here keeps the JSON readable by anything.
    for tag, patch in overrides.items():
        merged = dict(tables["en"])
        merged.update(patch)
        tables[tag] = merged
    return {tag: tables[tag] for _, tag in LANGS}


def json_output(tables: dict[str, dict[str, str]]) -> dict[Path, str]:
    out = {}
    for tag, table in tables.items():
        out[JSON_DIR / f"{tag}.json"] = (
            json.dumps(table, ensure_ascii=False, indent=2, sort_keys=False) + "\n"
        )
    return out


def swift_output(tables: dict[str, dict[str, str]]) -> str:
    lines = [
        "// Generated by scripts/ci/strings-codegen.sh — do not edit.",
        "//",
        "// The tables used to be typed inline in L10n.swift; #239 AC 4 moved them to",
        "// shared/contract/strings/*.json so the desktop reads the same strings macOS",
        "// draws. `./scripts/ci/strings-codegen.sh --check` fails when either side",
        "// drifts.",
        "//",
        "// de/ja/ko/tr are stored resolved: L10n.subscript already falls back to `en`",
        "// for a missing key, so the merge only moves work from a lookup to this file.",
        "",
        "extension L10n {",
    ]
    for var, tag in LANGS:
        lines.append(f"    static let {var}: [String: String] = [")
        for key, value in tables[tag].items():
            lines.append(f'        "{escape_swift(key)}": "{escape_swift(value)}",')
        lines.append("    ]")
        lines.append("")
    if lines[-1] == "":
        lines.pop()
    lines.append("}")
    return "\n".join(lines) + "\n"


def ts_output(tables: dict[str, dict[str, str]]) -> str:
    lines = [
        "// Generated by scripts/ci/strings-codegen.sh — do not edit.",
        "//",
        "// #239 AC 4: the same file macOS draws from (shared/contract/strings/).",
        "// `./scripts/ci/strings-codegen.sh --check` fails when either side drifts.",
        "",
        "export type Strings = Record<string, string>;",
        "",
        "export const STRINGS: Record<string, Strings> = {",
    ]
    for _, tag in LANGS:
        lines.append(f"  {json.dumps(tag)}: {{")
        for key, value in tables[tag].items():
            lines.append(f"    {json.dumps(key, ensure_ascii=False)}: {escape_ts(value)},")
        lines.append("  },")
    lines.append("};")
    lines.append("")
    return "\n".join(lines)


def load() -> dict[str, dict[str, str]]:
    """The JSON is authoritative; Swift is generated from it, never the reverse."""
    tables = {}
    for _, tag in LANGS:
        path = JSON_DIR / f"{tag}.json"
        if not path.exists():
            raise SystemExit(f"missing {path.relative_to(ROOT)} — run --extract once")
        tables[tag] = json.loads(path.read_text(encoding="utf-8"))
    return tables


def main(argv: list[str]) -> int:
    check = "--check" in argv
    if "--extract" in argv:
        # One-off: the tables still live in L10n.swift. After the first run the
        # JSON is the source of truth and this flag has nothing to read.
        tables = extract()
        JSON_DIR.mkdir(parents=True, exist_ok=True)
        for tag, table in tables.items():
            (JSON_DIR / f"{tag}.json").write_text(
                json.dumps(table, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
            )
        print(f"extracted {len(tables)} languages from L10n.swift")
        return 0
    tables = load()
    wanted: dict[Path, str] = {}
    wanted.update(json_output(tables))
    wanted[SWIFT_OUT] = swift_output(tables)
    wanted[TS_OUT] = ts_output(tables)

    stale = []
    for path, content in wanted.items():
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current == content:
            continue
        stale.append(path)
        if not check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

    if check:
        if stale:
            print("strings: generated files are out of date:", file=sys.stderr)
            for path in stale:
                print(f"  {path.relative_to(ROOT)}", file=sys.stderr)
            print("run ./scripts/ci/strings-codegen.sh", file=sys.stderr)
            return 1
        print(f"strings: {len(wanted)} files match {len(tables)} languages")
        return 0

    print(f"strings: wrote {len(stale)} of {len(wanted)} files")
    for tag, table in tables.items():
        print(f"  {tag:8s} {len(table):4d} keys")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
