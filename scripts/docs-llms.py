#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml"]
# ///
"""
Generate docs/_published/llms.txt and docs/_published/llms-full.txt from docs/src/.

Usage:
    uv run scripts/docs-llms.py [--src docs/src] [--out docs/_published] [--check]

--check: exit 1 if the output would differ from committed files; print diff to stderr.
"""

import argparse
import re
import sys
from pathlib import Path

import yaml

REPO_NAME = "mnemra-core"
REPO_DESCRIPTION = (
    "Plugin-extensible context layer for AI coding workflows. "
    "This site documents the project's intent, architecture decisions, and specifications."
)

# Regex for YAML frontmatter block at the start of a file.
# Matches ---\n ... \n--- (non-greedy, any chars including newlines).
_FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)


def parse_frontmatter(text: str) -> dict:
    """Return parsed YAML frontmatter dict, or empty dict if none."""
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1))
        return data if isinstance(data, dict) else {}
    except yaml.YAMLError:
        return {}


def parse_summary(summary_path: Path) -> list[tuple[str, str, str | None]]:
    """
    Parse SUMMARY.md and return a list of (title, relative_path, section_header) tuples.

    section_header is the `# Foo` section that precedes the entry, or None for entries
    before the first # header (these get synthetic "Introduction" section).

    Returns list of (title, rel_path, section) in SUMMARY.md order.
    """
    text = summary_path.read_text()
    entries = []
    current_section: str | None = None

    for line in text.splitlines():
        # Section header line: # Foo (not the # Summary title — we treat anything
        # other than the initial "# Summary" as a real section)
        if line.startswith("# ") and not line.startswith("## "):
            header = line[2:].strip()
            # "Summary" is the SUMMARY.md structural title, not a real section
            if header != "Summary":
                current_section = header
            continue

        # Entry line: - [Title](path.md)
        m = re.match(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\)\s*$", line)
        if m:
            title = m.group(1)
            rel_path = m.group(2)
            entries.append((title, rel_path, current_section))

    return entries


def generate_llms_txt(
    entries: list[tuple[str, str, str | None]],
    src_dir: Path,
) -> str:
    """
    Build llms.txt content from parsed SUMMARY.md entries.

    Raises SystemExit(1) if any entry has an empty summary.
    """
    lines = []
    lines.append(f"# {REPO_NAME}")
    lines.append("")
    lines.append(f"> {REPO_DESCRIPTION}")
    lines.append("")

    current_section: str | None = None

    for title, rel_path, section in entries:
        # Determine section header for this entry
        effective_section = section if section is not None else "Introduction"

        if effective_section != current_section:
            if current_section is not None:
                lines.append("")
            lines.append(f"## {effective_section}")
            lines.append("")
            current_section = effective_section

        # Read page frontmatter for summary
        page_path = src_dir / rel_path
        if not page_path.exists():
            print(f"error: SUMMARY.md references '{rel_path}' but file does not exist: {page_path}", file=sys.stderr)
            sys.exit(1)

        fm = parse_frontmatter(page_path.read_text())
        summary = fm.get("summary") or ""
        if not summary or not summary.strip():
            print(
                f"error: page '{rel_path}' has empty 'summary:' frontmatter field. "
                "All SUMMARY.md-referenced pages must have a non-empty summary.",
                file=sys.stderr,
            )
            sys.exit(1)

        lines.append(f"- [{title}]({rel_path}): {summary.strip()}")

    lines.append("")
    return "\n".join(lines)


def generate_llms_full_txt(
    entries: list[tuple[str, str, str | None]],
    src_dir: Path,
) -> str:
    """
    Build llms-full.txt: concatenation of every .md file under src_dir
    (except SUMMARY.md) in SUMMARY.md order, then non-SUMMARY pages in
    lexicographic path order.

    Each page is preceded by: <!-- ===== docs/src/relative/path.md ===== -->
    """
    # Build ordered list from SUMMARY
    summary_paths = []
    seen: set[str] = set()
    for _title, rel_path, _section in entries:
        if rel_path not in seen:
            summary_paths.append(rel_path)
            seen.add(rel_path)

    # Collect all .md files not in SUMMARY (excluding SUMMARY.md itself)
    all_md = sorted(
        str(p.relative_to(src_dir))
        for p in src_dir.rglob("*.md")
        if p.name != "SUMMARY.md"
    )
    tail_paths = [p for p in all_md if p not in seen]

    ordered = summary_paths + tail_paths

    parts = []
    for rel_path in ordered:
        page_path = src_dir / rel_path
        if not page_path.exists():
            print(f"error: referenced page does not exist: {page_path}", file=sys.stderr)
            sys.exit(1)
        content = page_path.read_text()
        # Normalize: strip trailing whitespace from content, ensure single trailing newline
        content = content.rstrip("\n")
        separator = f"<!-- ===== docs/src/{rel_path} ===== -->"
        parts.append(f"{separator}\n\n{content}")

    return "\n\n".join(parts) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate llms.txt + llms-full.txt from docs/src/")
    parser.add_argument(
        "--src",
        type=Path,
        default=Path("docs/src"),
        help="Path to docs/src directory (default: docs/src)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("docs/_published"),
        help="Output directory (default: docs/_published)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit 1 if outputs would differ from committed files; print diff to stderr.",
    )
    args = parser.parse_args()

    src_dir: Path = args.src
    out_dir: Path = args.out

    if not src_dir.exists():
        print(f"error: src directory does not exist: {src_dir}", file=sys.stderr)
        sys.exit(1)

    summary_path = src_dir / "SUMMARY.md"
    if not summary_path.exists():
        print(f"error: SUMMARY.md not found at {summary_path}", file=sys.stderr)
        sys.exit(1)

    entries = parse_summary(summary_path)
    llms_txt = generate_llms_txt(entries, src_dir)
    llms_full_txt = generate_llms_full_txt(entries, src_dir)

    if args.check:
        drift = False
        for filename, new_content in [("llms.txt", llms_txt), ("llms-full.txt", llms_full_txt)]:
            committed_path = out_dir / filename
            if not committed_path.exists():
                print(f"drift: {filename} does not exist in {out_dir}", file=sys.stderr)
                drift = True
            else:
                committed = committed_path.read_text()
                if committed != new_content:
                    print(f"drift: {filename} differs from what generator would produce", file=sys.stderr)
                    # Print a brief diff summary
                    committed_lines = committed.splitlines()
                    new_lines = new_content.splitlines()
                    if len(committed_lines) != len(new_lines):
                        print(
                            f"  committed: {len(committed_lines)} lines, "
                            f"generated: {len(new_lines)} lines",
                            file=sys.stderr,
                        )
                    drift = True
        if drift:
            print(
                "Run 'just docs-llms' to regenerate, then commit the updated outputs.",
                file=sys.stderr,
            )
            sys.exit(1)
        return

    # Write mode: create out_dir if needed and write outputs
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "llms.txt").write_text(llms_txt)
    (out_dir / "llms-full.txt").write_text(llms_full_txt)


if __name__ == "__main__":
    main()
