#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml"]
# ///
"""
Translation pipeline for docs/_published/{human,agent}/ trees.

Modes:
  --plan     Compute stale pages, emit JSON plan to stdout, perform verbatim copies,
             write pending sidecar. Never mutates the manifest.
  --finalize Validate that translated outputs exist, apply manifest mutations atomically,
             delete pending sidecar on success.
  --check    Read-only drift gate. Exits 0 if manifest matches reality; 1 otherwise.

Usage:
    uv run scripts/docs-translate.py {--plan | --finalize | --check} \\
        [--src docs/src] [--out docs/_published] [--prompts docs/prompts]
"""

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

import yaml

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

MANIFEST_FILENAME = ".translation-manifest.json"
PENDING_FILENAME = ".translation-pending.json"
MANIFEST_SCHEMA_VERSION = 1
PENDING_SCHEMA_VERSION = 1

VALID_AUDIENCES = {"agent", "human"}
VALID_PROMPT_PATHS = {"explain-pass.md", "strip-pass.md"}
# Audience → which prompt produces the OPPOSITE-audience form
AUDIENCE_TO_PROMPT = {
    "agent": "explain-pass.md",  # agent-primary: translate to human via EXPLAIN
    "human": "strip-pass.md",    # human-primary: translate to agent via STRIP
}
# Audience → which side is the verbatim copy
AUDIENCE_TO_VERBATIM_SIDE = {
    "agent": "agent",
    "human": "human",
}
# Audience → which side is the translated output
AUDIENCE_TO_TRANSLATED_SIDE = {
    "agent": "human",
    "human": "agent",
}

# Regex for YAML frontmatter block at the start of a file.
_FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)

# 64-char lowercase hex
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


# ---------------------------------------------------------------------------
# Hashing
# ---------------------------------------------------------------------------

def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


# ---------------------------------------------------------------------------
# Frontmatter parsing
# ---------------------------------------------------------------------------

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


def strip_frontmatter(text: str) -> str:
    """Return text with the first frontmatter block removed."""
    m = _FRONTMATTER_RE.match(text)
    if m:
        return text[m.end():]
    return text


# ---------------------------------------------------------------------------
# Manifest IO
# ---------------------------------------------------------------------------

def read_manifest(manifest_path: Path) -> dict | None:
    """Return parsed manifest dict, or None if the file does not exist."""
    if not manifest_path.exists():
        return None
    return json.loads(manifest_path.read_text())


def write_manifest(manifest_path: Path, manifest: dict) -> None:
    """Write manifest atomically (temp file + rename)."""
    os.makedirs(manifest_path.parent, exist_ok=True)
    tmp = manifest_path.with_suffix(".json.tmp")
    tmp.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    tmp.replace(manifest_path)


def validate_manifest(manifest: dict) -> list[str]:
    """
    Validate manifest schema. Return list of error strings (empty = valid).

    Checks for all entries: schema_version, required fields, valid audience,
    valid prompt_path, audience↔prompt agreement, and SHA-256 hex format.
    """
    errors: list[str] = []

    if manifest.get("schema_version") != MANIFEST_SCHEMA_VERSION:
        errors.append(
            f"schema_version: expected {MANIFEST_SCHEMA_VERSION}, "
            f"got {manifest.get('schema_version')!r}"
        )
        return errors  # can't trust entries structure if version is wrong

    entries = manifest.get("entries", {})
    required_fields = {
        "source_sha256", "prompt_path", "prompt_sha256",
        "primary_audience", "translated_at",
    }

    for rel_path, entry in entries.items():
        missing = required_fields - set(entry.keys())
        if missing:
            errors.append(f"entry '{rel_path}': missing fields: {sorted(missing)}")
            continue

        sha = entry["source_sha256"]
        if not _SHA256_RE.match(sha):
            errors.append(
                f"entry '{rel_path}': source_sha256 not 64 lowercase hex chars: {sha!r}"
            )

        psha = entry["prompt_sha256"]
        if not _SHA256_RE.match(psha):
            errors.append(
                f"entry '{rel_path}': prompt_sha256 not 64 lowercase hex chars: {psha!r}"
            )

        audience = entry["primary_audience"]
        if audience not in VALID_AUDIENCES:
            errors.append(
                f"entry '{rel_path}': primary_audience must be 'agent' or 'human', "
                f"got {audience!r}"
            )

        prompt_path = entry["prompt_path"]
        if prompt_path not in VALID_PROMPT_PATHS:
            errors.append(
                f"entry '{rel_path}': prompt_path must be 'explain-pass.md' or "
                f"'strip-pass.md', got {prompt_path!r}"
            )

        # Audience ↔ prompt agreement: agent → explain, human → strip
        if audience in VALID_AUDIENCES and prompt_path in VALID_PROMPT_PATHS:
            expected_prompt = AUDIENCE_TO_PROMPT.get(audience)
            if expected_prompt and prompt_path != expected_prompt:
                errors.append(
                    f"entry '{rel_path}': primary_audience '{audience}' disagrees with "
                    f"prompt_path '{prompt_path}' (expected '{expected_prompt}')"
                )

    return errors


# ---------------------------------------------------------------------------
# Discover source pages
# ---------------------------------------------------------------------------

def collect_source_pages(src_dir: Path) -> tuple[list[Path], Path | None]:
    """
    Return (pages, summary_path) where pages are all .md files under src_dir
    (excluding SUMMARY.md and glossary.md, which are structural/injection-only).
    summary_path is None if SUMMARY.md not found.
    """
    summary = src_dir / "SUMMARY.md"
    # Excluded from translation: SUMMARY.md (structural) and glossary.md (glossary injection source)
    excluded = {"SUMMARY.md", "glossary.md"}
    pages = [
        p for p in sorted(src_dir.rglob("*.md"))
        if p.name not in excluded
    ]
    return pages, (summary if summary.exists() else None)


# ---------------------------------------------------------------------------
# Prompt assembly
# ---------------------------------------------------------------------------

def validate_prompt(prompt_path: Path) -> list[str]:
    """Return list of error strings if prompt is missing required tokens."""
    text = prompt_path.read_text()
    errors = []
    if "{{GLOSSARY}}" not in text:
        errors.append(f"prompt '{prompt_path.name}' missing {{{{GLOSSARY}}}} token")
    if "{{PAGE}}" not in text:
        errors.append(f"prompt '{prompt_path.name}' missing {{{{PAGE}}}} token")
    if "{{NONCE}}" not in text:
        errors.append(f"prompt '{prompt_path.name}' missing {{{{NONCE}}}} token")
    return errors


def make_prompt_text(prompt_path: Path, glossary_body: str, page_text: str) -> str:
    """Substitute {{NONCE}}, {{GLOSSARY}}, and {{PAGE}} into prompt template.

    Nonce is substituted first so that content tokens cannot inject {{NONCE}}
    literals into wrapper tag positions.
    """
    nonce = secrets.token_hex(8)
    text = prompt_path.read_text()
    text = text.replace("{{NONCE}}", nonce)
    text = text.replace("{{GLOSSARY}}", glossary_body)
    text = text.replace("{{PAGE}}", page_text)
    return text


# ---------------------------------------------------------------------------
# Path helpers
# ---------------------------------------------------------------------------

def rel_to_src(page_path: Path, src_dir: Path) -> str:
    """Return the path of page relative to src_dir as a POSIX string."""
    return str(page_path.relative_to(src_dir))


def verbatim_dest(rel: str, audience: str, out_dir: Path) -> Path:
    """The verbatim copy destination for a page (canonical-audience side)."""
    side = AUDIENCE_TO_VERBATIM_SIDE[audience]
    return out_dir / side / rel


def translated_dest(rel: str, audience: str, out_dir: Path) -> Path:
    """The translated output destination for a page (opposite-audience side)."""
    side = AUDIENCE_TO_TRANSLATED_SIDE[audience]
    return out_dir / side / rel


# ---------------------------------------------------------------------------
# --plan mode
# ---------------------------------------------------------------------------

def cmd_plan(args: argparse.Namespace) -> int:
    src_dir = Path(args.src)
    out_dir = Path(args.out)
    prompts_dir = Path(args.prompts)

    if not src_dir.exists():
        print(f"error: src directory does not exist: {src_dir}", file=sys.stderr)
        return 1

    # Load prompt files and validate tokens
    explain_path = prompts_dir / "explain-pass.md"
    strip_path = prompts_dir / "strip-pass.md"
    for prompt_file in (explain_path, strip_path):
        if not prompt_file.exists():
            print(f"error: prompt file not found: {prompt_file}", file=sys.stderr)
            return 1
        errs = validate_prompt(prompt_file)
        if errs:
            for e in errs:
                print(f"error: {e}", file=sys.stderr)
            return 1

    # Load and strip glossary frontmatter
    glossary_path = src_dir / "glossary.md"
    if not glossary_path.exists():
        print(f"error: glossary.md not found at {glossary_path}", file=sys.stderr)
        return 1
    glossary_text = glossary_path.read_text()
    glossary_body = strip_frontmatter(glossary_text)

    # Pre-compute prompt hashes
    prompt_hashes: dict[str, str] = {
        "explain-pass.md": sha256_file(explain_path),
        "strip-pass.md": sha256_file(strip_path),
    }

    # Read manifest (may not exist on first run)
    manifest_path = out_dir / MANIFEST_FILENAME
    manifest = read_manifest(manifest_path)
    # Validation deferred until after collect_source_pages (to know which entries are orphans)
    manifest_entries = manifest.get("entries", {}) if manifest is not None else {}

    pages, summary_path = collect_source_pages(src_dir)
    src_pages_rel = {rel_to_src(p, src_dir) for p in pages}

    # Validate manifest
    if manifest is not None:
        errs = validate_manifest(manifest)
        if errs:
            for e in errs:
                print(f"error: {e}", file=sys.stderr)
            return 1

    # -- SUMMARY.md: copy to both sides --
    if summary_path is not None:
        summary_content = summary_path.read_bytes()
        for side in ("human", "agent"):
            dest = out_dir / side / "SUMMARY.md"
            os.makedirs(dest.parent, exist_ok=True)
            dest.write_bytes(summary_content)

    # -- Detect orphan manifest entries (source file deleted) --
    orphans: list[str] = []
    for rel_key in list(manifest_entries.keys()):
        if rel_key not in src_pages_rel:
            orphans.append(rel_key)
            # Remove orphan published files from both sides
            for side in ("human", "agent"):
                orphan_pub = out_dir / side / rel_key
                if orphan_pub.exists():
                    orphan_pub.unlink()
                    print(f"removed orphan: {orphan_pub}", file=sys.stderr)

    # -- Build plan items + verbatim copies --
    plan_items: list[dict] = []

    for page_path in pages:
        rel = rel_to_src(page_path, src_dir)
        page_text = page_path.read_text()
        fm = parse_frontmatter(page_text)

        audience = fm.get("primary-audience")
        if not audience:
            print(
                f"error: page '{rel}' has no 'primary-audience' frontmatter field",
                file=sys.stderr,
            )
            return 1
        if audience not in VALID_AUDIENCES:
            print(
                f"error: page '{rel}' has invalid primary-audience: {audience!r}",
                file=sys.stderr,
            )
            return 1

        # Verbatim copy to canonical-audience side
        verbatim = verbatim_dest(rel, audience, out_dir)
        os.makedirs(verbatim.parent, exist_ok=True)
        shutil.copy2(str(page_path), str(verbatim))

        # Determine if translation is needed
        prompt_name = AUDIENCE_TO_PROMPT[audience]
        current_source_sha = sha256_file(page_path)
        current_prompt_sha = prompt_hashes[prompt_name]
        translated = translated_dest(rel, audience, out_dir)

        entry = manifest_entries.get(rel)
        if (
            entry is not None
            and entry.get("source_sha256") == current_source_sha
            and entry.get("prompt_sha256") == current_prompt_sha
            and translated.exists()
        ):
            # Up-to-date: skip
            continue

        # Stale or missing: include in plan
        if audience == "agent":
            prompt_path = explain_path
        else:
            prompt_path = strip_path

        assembled = make_prompt_text(prompt_path, glossary_body, page_text)

        plan_items.append({
            "source_path": str(page_path),
            "output_path": str(translated),
            "primary_audience": audience,
            "prompt_path": str(prompt_path),
            "source_sha256": current_source_sha,
            "prompt_sha256": current_prompt_sha,
            "assembled_prompt": assembled,
        })

    # -- Write pending sidecar --
    pending = {
        "schema_version": PENDING_SCHEMA_VERSION,
        "items": [
            {
                "source_path": item["source_path"],
                "output_path": item["output_path"],
                "primary_audience": item["primary_audience"],
                "prompt_path": item["prompt_path"],
                "source_sha256": item["source_sha256"],
                "prompt_sha256": item["prompt_sha256"],
            }
            for item in plan_items
        ],
        "orphans_to_remove": orphans,
    }
    os.makedirs(out_dir, exist_ok=True)
    pending_path = out_dir / PENDING_FILENAME
    pending_path.write_text(json.dumps(pending, indent=2) + "\n")

    # -- Emit plan JSON to stdout (manifest NOT touched) --
    plan_out = {
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "items": plan_items,
    }
    print(json.dumps(plan_out, indent=2))
    return 0


# ---------------------------------------------------------------------------
# --finalize mode
# ---------------------------------------------------------------------------

def cmd_finalize(args: argparse.Namespace) -> int:
    out_dir = Path(args.out)
    src_dir = Path(args.src)

    manifest_path = out_dir / MANIFEST_FILENAME
    pending_path = out_dir / PENDING_FILENAME

    if not pending_path.exists():
        print(f"error: pending sidecar not found: {pending_path}", file=sys.stderr)
        return 1

    pending = json.loads(pending_path.read_text())
    items = pending.get("items", [])
    orphans_to_remove: list[str] = pending.get("orphans_to_remove", [])

    # Validate all outputs exist and are non-empty before touching manifest
    failed: list[str] = []
    for item in items:
        output = Path(item["output_path"])
        if not output.exists():
            print(f"error: missing output: {output}", file=sys.stderr)
            failed.append(str(output))
        elif output.stat().st_size == 0:
            print(f"error: empty output: {output}", file=sys.stderr)
            failed.append(str(output))

    if failed:
        # Leave both manifest and sidecar unchanged
        return 1

    # Read existing manifest (may not exist)
    manifest = read_manifest(manifest_path)
    if manifest is None:
        manifest = {"schema_version": MANIFEST_SCHEMA_VERSION, "entries": {}}
    entries = manifest.get("entries", {})

    now_ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    # Add/update entries for translated items
    for item in items:
        source_path = Path(item["source_path"])
        rel = str(source_path.resolve().relative_to(src_dir.resolve()))

        # prompt_path in sidecar is absolute; manifest stores filename only
        prompt_name = Path(item["prompt_path"]).name
        audience = item["primary_audience"]

        entries[rel] = {
            "primary_audience": audience,
            "prompt_path": prompt_name,
            "prompt_sha256": item["prompt_sha256"],
            "source_sha256": item["source_sha256"],
            "translated_at": now_ts,
        }

    # Remove orphan entries
    for orphan_rel in orphans_to_remove:
        entries.pop(orphan_rel, None)

    manifest["entries"] = entries
    write_manifest(manifest_path, manifest)

    # Remove pending sidecar
    pending_path.unlink()
    return 0


# ---------------------------------------------------------------------------
# --check mode
# ---------------------------------------------------------------------------

def cmd_check(args: argparse.Namespace) -> int:
    src_dir = Path(args.src)
    out_dir = Path(args.out)
    prompts_dir = Path(args.prompts)

    if not src_dir.exists():
        print(f"error: src directory does not exist: {src_dir}", file=sys.stderr)
        return 1

    manifest_path = out_dir / MANIFEST_FILENAME
    manifest = read_manifest(manifest_path)
    if manifest is None:
        print(f"drift: manifest not found: {manifest_path}", file=sys.stderr)
        return 1

    pages, _ = collect_source_pages(src_dir)
    src_pages_rel = {rel_to_src(p, src_dir) for p in pages}

    errs = validate_manifest(manifest)
    if errs:
        for e in errs:
            print(f"error: {e}", file=sys.stderr)
        return 1

    manifest_entries = manifest.get("entries", {})

    # Pre-compute current prompt hashes
    explain_path = prompts_dir / "explain-pass.md"
    strip_path = prompts_dir / "strip-pass.md"
    prompt_hashes: dict[str, str] = {}
    for name, p in (("explain-pass.md", explain_path), ("strip-pass.md", strip_path)):
        if p.exists():
            prompt_hashes[name] = sha256_file(p)

    drift = False

    # Check for orphan manifest entries (source deleted)
    for rel_key in manifest_entries:
        if rel_key not in src_pages_rel:
            print(f"drift: orphan manifest entry (source deleted): {rel_key}", file=sys.stderr)
            drift = True

    # Check each source page against manifest
    for page_path in pages:
        rel = rel_to_src(page_path, src_dir)
        page_text = page_path.read_text()
        fm = parse_frontmatter(page_text)
        audience = fm.get("primary-audience")

        if not audience or audience not in VALID_AUDIENCES:
            print(
                f"drift: page '{rel}' has missing or invalid primary-audience",
                file=sys.stderr,
            )
            drift = True
            continue

        prompt_name = AUDIENCE_TO_PROMPT[audience]
        current_source_sha = sha256_file(page_path)
        current_prompt_sha = prompt_hashes.get(prompt_name, "")

        entry = manifest_entries.get(rel)
        if entry is None:
            print(f"drift: page '{rel}' not in manifest", file=sys.stderr)
            drift = True
            continue

        if entry.get("source_sha256") != current_source_sha:
            print(f"drift: stale source hash for page.md: {rel}", file=sys.stderr)
            drift = True

        if entry.get("prompt_sha256") != current_prompt_sha:
            print(
                f"drift: stale prompt hash for {prompt_name} "
                f"(affects: {rel})",
                file=sys.stderr,
            )
            drift = True

        # Check both published files exist
        agent_pub = out_dir / "agent" / rel
        human_pub = out_dir / "human" / rel
        for pub in (agent_pub, human_pub):
            if not pub.exists():
                print(f"drift: missing published file: {pub}", file=sys.stderr)
                drift = True

    return 1 if drift else 0


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Translation pipeline for docs/_published/{human,agent}/ trees."
    )
    mode_group = parser.add_mutually_exclusive_group(required=True)
    mode_group.add_argument(
        "--plan",
        action="store_true",
        help="Compute translation plan, write verbatim copies and pending sidecar.",
    )
    mode_group.add_argument(
        "--finalize",
        action="store_true",
        help="Validate outputs and update manifest.",
    )
    mode_group.add_argument(
        "--check",
        action="store_true",
        help="Read-only drift gate. Exit 0 if manifest matches reality.",
    )
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
        "--prompts",
        type=Path,
        default=Path("docs/prompts"),
        help="Prompts directory (default: docs/prompts)",
    )

    args = parser.parse_args()

    if args.plan:
        sys.exit(cmd_plan(args))
    elif args.finalize:
        sys.exit(cmd_finalize(args))
    elif args.check:
        sys.exit(cmd_check(args))


if __name__ == "__main__":
    main()
