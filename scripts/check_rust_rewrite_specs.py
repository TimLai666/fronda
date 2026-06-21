#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC_DIR = ROOT / "specs" / "rust-rewrite"

REQUIRED_FILES = [
    "README.md",
    "00-runtime-packaging-design-and-shell.md",
    "01-foundation-and-project-model.md",
    "02-media-library-and-project-workflows.md",
    "03-timeline-editor-and-preview.md",
    "04-export-rendering-and-interchange.md",
    "05-agent-mcp-and-chat.md",
    "06-search-transcription-generation-and-shell.md",
    "98-verification-plan.md",
    "99-test-matrix.md",
]

CHECKLIST_FILES = [
    "00-runtime-packaging-design-and-shell.md",
    "01-foundation-and-project-model.md",
    "02-media-library-and-project-workflows.md",
    "03-timeline-editor-and-preview.md",
    "04-export-rendering-and-interchange.md",
    "05-agent-mcp-and-chat.md",
    "06-search-transcription-generation-and-shell.md",
]

CHECKLIST_ITEM_RE = re.compile(
    r"^- \[(?P<mark>[ xX-])\] `(?P<id>[A-Z]+-\d{3})`:.*$", re.MULTILINE
)


def line_number_for_offset(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def main() -> int:
    errors: list[str] = []

    if not SPEC_DIR.exists():
        print(f"error: spec directory not found: {SPEC_DIR}")
        return 1

    missing_files = [name for name in REQUIRED_FILES if not (SPEC_DIR / name).exists()]
    if missing_files:
        errors.append(
            "missing required spec files:\n  - " + "\n  - ".join(missing_files)
        )

    readme_path = SPEC_DIR / "README.md"
    readme_text = (
        readme_path.read_text(encoding="utf-8") if readme_path.exists() else ""
    )
    for filename in REQUIRED_FILES:
        if filename == "README.md":
            continue
        if filename not in readme_text:
            errors.append(f"README.md does not mention required spec file `{filename}`")

    checklist_counts: dict[str, int] = {}
    family_counts: Counter[str] = Counter()
    locations_by_id: dict[str, list[str]] = defaultdict(list)

    for filename in CHECKLIST_FILES:
        path = SPEC_DIR / filename
        if not path.exists():
            continue

        text = path.read_text(encoding="utf-8")
        matches = list(CHECKLIST_ITEM_RE.finditer(text))
        checklist_counts[filename] = len(matches)

        if not matches:
            errors.append(f"{filename} does not contain any checklist items")
            continue

        for match in matches:
            spec_id = match.group("id")
            family = spec_id.split("-", 1)[0]
            family_counts[family] += 1
            line = line_number_for_offset(text, match.start())
            locations_by_id[spec_id].append(f"{filename}:{line}")

    duplicate_ids = {
        spec_id: locations
        for spec_id, locations in locations_by_id.items()
        if len(locations) > 1
    }
    if duplicate_ids:
        lines = ["duplicate checklist IDs found:"]
        for spec_id in sorted(duplicate_ids):
            joined = ", ".join(duplicate_ids[spec_id])
            lines.append(f"  - {spec_id}: {joined}")
        errors.append("\n".join(lines))

    if errors:
        print("Rust rewrite spec validation failed:\n")
        for error in errors:
            print(f"- {error}")
        return 1

    total_ids = sum(checklist_counts.values())
    print("Rust rewrite spec validation passed")
    print(f"- required files: {len(REQUIRED_FILES)}")
    print(f"- checklist files: {len(CHECKLIST_FILES)}")
    print(f"- checklist IDs: {total_ids}")
    print(f"- checklist families: {len(family_counts)}")
    print("\nChecklist coverage by file:")
    for filename in CHECKLIST_FILES:
        count = checklist_counts.get(filename, 0)
        print(f"- {filename}: {count}")

    print("\nChecklist families:")
    for family in sorted(family_counts):
        print(f"- {family}: {family_counts[family]}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
