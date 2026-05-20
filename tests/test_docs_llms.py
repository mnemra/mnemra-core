"""
Tests for scripts/docs-llms.py generator.

Tests are fixture-based: each test builds a temporary docs/src/ tree and
calls the generator against it, asserting exact output (string equality for
golden tests, exit codes for mode tests).
"""

import os
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).parent.parent
GENERATOR = REPO_ROOT / "scripts" / "docs-llms.py"


def run_generator(src_dir: Path, out_dir: Path, extra_args: list[str] = None) -> subprocess.CompletedProcess:
    """Run the generator with uv, pointing at a fixture src_dir."""
    args = [
        "uv",
        "run",
        str(GENERATOR),
        "--src", str(src_dir),
        "--out", str(out_dir),
    ]
    if extra_args:
        args.extend(extra_args)
    return subprocess.run(args, capture_output=True, text=True)


def make_page(path: Path, title: str, summary: str, body: str = "", audience: str = "human") -> None:
    """Write a markdown page with YAML frontmatter."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fm_summary = f'"{summary}"' if summary else ""
    content = f"""---
title: "{title}"
summary: {fm_summary}
primary-audience: {audience}
---

{body}
""".lstrip()
    path.write_text(content)


def make_summary(path: Path, content: str) -> None:
    """Write a SUMMARY.md file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


# ---------------------------------------------------------------------------
# Test 1: Golden llms.txt
# ---------------------------------------------------------------------------

def test_golden_llms_txt(tmp_path):
    """Fixture docs/src/ tree produces exact expected llms.txt (string equality)."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    # Build minimal fixture matching the real docs/src structure
    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Introduction](intro.md)

        # Intent

        - [mnemra-core](intent/mnemra-core.md)

        # Specifications

        - [About this section](specs/README.md)

        # Architecture Decision Records

        - [ADR Overview](adrs/README.md)
        - [Project Defaults](adrs/DEFAULTS.md)
        - [ADR Template](adrs/template.md)

        # Glossary

        - [Glossary](glossary.md)
    """))

    make_page(src / "intro.md", "mnemra-core", "Entry point for the mnemra-core documentation site.")
    make_page(src / "intent/mnemra-core.md", "Product Brief: Mnemra Core",
              "Product brief locking mnemra-core's intent and feature register.", audience="agent")
    make_page(src / "specs/README.md", "Specifications",
              "How canonical architecture specs differ from implementation-history specs in this repo.")
    make_page(src / "adrs/README.md", "Architecture Decision Records",
              "How architecture decisions are structured here (G-* / P-* prefixes, MADR format).")
    make_page(src / "adrs/DEFAULTS.md", "Project Standards",
              "Project defaults projected from workspace G-* canon — baseline for mnemra-core.", audience="agent")
    make_page(src / "adrs/template.md", "P-NNNN: Title",
              "MADR template for authoring new P-NNNN ADRs.", audience="agent")
    make_page(src / "glossary.md", "Glossary",
              "Terms and conventions used across mnemra-core's intent, ADRs, and specs.")

    result = run_generator(src, out)
    assert result.returncode == 0, f"Generator failed:\n{result.stderr}"

    expected = textwrap.dedent("""\
        # mnemra-core

        > Plugin-extensible context layer for AI coding workflows. This site documents the project's intent, architecture decisions, and specifications.

        ## Introduction

        - [Introduction](intro.md): Entry point for the mnemra-core documentation site.

        ## Intent

        - [mnemra-core](intent/mnemra-core.md): Product brief locking mnemra-core's intent and feature register.

        ## Specifications

        - [About this section](specs/README.md): How canonical architecture specs differ from implementation-history specs in this repo.

        ## Architecture Decision Records

        - [ADR Overview](adrs/README.md): How architecture decisions are structured here (G-* / P-* prefixes, MADR format).
        - [Project Defaults](adrs/DEFAULTS.md): Project defaults projected from workspace G-* canon — baseline for mnemra-core.
        - [ADR Template](adrs/template.md): MADR template for authoring new P-NNNN ADRs.

        ## Glossary

        - [Glossary](glossary.md): Terms and conventions used across mnemra-core's intent, ADRs, and specs.
    """)

    actual = (out / "llms.txt").read_text()
    assert actual == expected, f"llms.txt mismatch.\nExpected:\n{expected}\nActual:\n{actual}"


# ---------------------------------------------------------------------------
# Test 2: Golden llms-full.txt
# ---------------------------------------------------------------------------

def test_golden_llms_full_txt(tmp_path):
    """Same fixture tree produces exact expected llms-full.txt (string equality)."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Introduction](intro.md)

        # Intent

        - [mnemra-core](intent/mnemra-core.md)

        # Specifications

        - [About this section](specs/README.md)

        # Architecture Decision Records

        - [ADR Overview](adrs/README.md)
        - [Project Defaults](adrs/DEFAULTS.md)
        - [ADR Template](adrs/template.md)

        # Glossary

        - [Glossary](glossary.md)
    """))

    intro_body = "Intro body text."
    intent_body = "Intent body text."
    specs_body = "Specs body text."
    adrs_body = "ADRs body text."
    defaults_body = "Defaults body text."
    template_body = "Template body text."
    glossary_body = "Glossary body text."

    make_page(src / "intro.md", "mnemra-core",
              "Entry point for the mnemra-core documentation site.", body=intro_body)
    make_page(src / "intent/mnemra-core.md", "Product Brief: Mnemra Core",
              "Product brief locking mnemra-core's intent and feature register.",
              body=intent_body, audience="agent")
    make_page(src / "specs/README.md", "Specifications",
              "How canonical architecture specs differ from implementation-history specs in this repo.",
              body=specs_body)
    make_page(src / "adrs/README.md", "Architecture Decision Records",
              "How architecture decisions are structured here (G-* / P-* prefixes, MADR format).",
              body=adrs_body)
    make_page(src / "adrs/DEFAULTS.md", "Project Standards",
              "Project defaults projected from workspace G-* canon — baseline for mnemra-core.",
              body=defaults_body, audience="agent")
    make_page(src / "adrs/template.md", "P-NNNN: Title",
              "MADR template for authoring new P-NNNN ADRs.",
              body=template_body, audience="agent")
    make_page(src / "glossary.md", "Glossary",
              "Terms and conventions used across mnemra-core's intent, ADRs, and specs.",
              body=glossary_body)

    result = run_generator(src, out)
    assert result.returncode == 0, f"Generator failed:\n{result.stderr}"

    actual = (out / "llms-full.txt").read_text()

    # Each page separator + content appears in SUMMARY.md order
    assert "<!-- ===== docs/src/intro.md ===== -->" in actual
    assert "<!-- ===== docs/src/intent/mnemra-core.md ===== -->" in actual
    assert "<!-- ===== docs/src/specs/README.md ===== -->" in actual
    assert "<!-- ===== docs/src/adrs/README.md ===== -->" in actual
    assert "<!-- ===== docs/src/adrs/DEFAULTS.md ===== -->" in actual
    assert "<!-- ===== docs/src/adrs/template.md ===== -->" in actual
    assert "<!-- ===== docs/src/glossary.md ===== -->" in actual

    # Verify order: intro before intent before specs before adrs/README before glossary
    pos_intro = actual.index("<!-- ===== docs/src/intro.md ===== -->")
    pos_intent = actual.index("<!-- ===== docs/src/intent/mnemra-core.md ===== -->")
    pos_specs = actual.index("<!-- ===== docs/src/specs/README.md ===== -->")
    pos_adrs = actual.index("<!-- ===== docs/src/adrs/README.md ===== -->")
    pos_glossary = actual.index("<!-- ===== docs/src/glossary.md ===== -->")

    assert pos_intro < pos_intent < pos_specs < pos_adrs < pos_glossary

    # Content appears after its separator
    assert intro_body in actual
    assert intent_body in actual
    assert glossary_body in actual


# ---------------------------------------------------------------------------
# Test 3: SUMMARY ordering preserved in llms-full.txt
# ---------------------------------------------------------------------------

def test_summary_order_preserved(tmp_path):
    """Pages in non-alphabetical SUMMARY order appear in that order in llms-full.txt."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    # SUMMARY order: zebra, alpha, mango — intentionally non-alphabetical
    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Zebra](zebra.md)
        - [Alpha](alpha.md)
        - [Mango](mango.md)
    """))

    make_page(src / "zebra.md", "Zebra", "Zebra summary.")
    make_page(src / "alpha.md", "Alpha", "Alpha summary.")
    make_page(src / "mango.md", "Mango", "Mango summary.")

    result = run_generator(src, out)
    assert result.returncode == 0, f"Generator failed:\n{result.stderr}"

    actual = (out / "llms-full.txt").read_text()

    pos_zebra = actual.index("<!-- ===== docs/src/zebra.md ===== -->")
    pos_alpha = actual.index("<!-- ===== docs/src/alpha.md ===== -->")
    pos_mango = actual.index("<!-- ===== docs/src/mango.md ===== -->")

    assert pos_zebra < pos_alpha < pos_mango, (
        f"SUMMARY order not preserved: zebra={pos_zebra}, alpha={pos_alpha}, mango={pos_mango}"
    )


# ---------------------------------------------------------------------------
# Test 4: Tail appending — non-SUMMARY pages appended at end in lexicographic order
# ---------------------------------------------------------------------------

def test_tail_appending(tmp_path):
    """A page not in SUMMARY appears after SUMMARY pages in llms-full.txt."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    # SUMMARY has only one page; extra.md is not in SUMMARY
    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Main](main.md)
    """))

    make_page(src / "main.md", "Main", "Main summary.")
    make_page(src / "extra.md", "Extra", "Extra summary.")

    result = run_generator(src, out)
    assert result.returncode == 0, f"Generator failed:\n{result.stderr}"

    actual = (out / "llms-full.txt").read_text()

    pos_main = actual.index("<!-- ===== docs/src/main.md ===== -->")
    pos_extra = actual.index("<!-- ===== docs/src/extra.md ===== -->")

    assert pos_main < pos_extra, (
        f"SUMMARY page must precede non-SUMMARY page: main={pos_main}, extra={pos_extra}"
    )


# ---------------------------------------------------------------------------
# Test 5: Empty summary causes generator to exit 1 with error
# ---------------------------------------------------------------------------

def test_empty_summary_exits_1(tmp_path):
    """A SUMMARY-referenced page with empty summary: causes generator exit 1."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Main](main.md)
    """))

    # Write page with empty summary
    page = src / "main.md"
    page.write_text(textwrap.dedent("""\
        ---
        title: "Main"
        summary:
        primary-audience: human
        ---

        Body.
    """))

    result = run_generator(src, out)
    assert result.returncode != 0, (
        f"Generator should exit nonzero for empty summary, but got 0.\nstderr: {result.stderr}"
    )
    assert "summary" in result.stderr.lower() or "summary" in result.stdout.lower(), (
        f"Error message should mention 'summary'.\nstdout: {result.stdout}\nstderr: {result.stderr}"
    )


# ---------------------------------------------------------------------------
# Test 6: --check exits 0 when committed outputs match regeneration
# ---------------------------------------------------------------------------

def test_check_clean(tmp_path):
    """--check exits 0 when _published/ matches what generator would produce."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Main](main.md)
    """))
    make_page(src / "main.md", "Main", "Main summary.")

    # First generate without --check to produce committed outputs
    result = run_generator(src, out)
    assert result.returncode == 0, f"Initial generation failed:\n{result.stderr}"

    # Now run --check; outputs already match → should exit 0
    result_check = run_generator(src, out, ["--check"])
    assert result_check.returncode == 0, (
        f"--check should exit 0 on clean tree.\nstdout: {result_check.stdout}\nstderr: {result_check.stderr}"
    )


# ---------------------------------------------------------------------------
# Test 7: --check exits 1 when committed outputs differ from regeneration
# ---------------------------------------------------------------------------

def test_check_drift(tmp_path):
    """--check exits 1 when _published/ has stale content that would be overwritten."""
    src = tmp_path / "src"
    out = tmp_path / "published"
    out.mkdir()

    make_summary(src / "SUMMARY.md", textwrap.dedent("""\
        # Summary

        - [Main](main.md)
    """))
    make_page(src / "main.md", "Main", "Main summary.")

    # Write a stale llms.txt (does not match what generator would produce)
    (out / "llms.txt").write_text("stale content that will not match\n")
    (out / "llms-full.txt").write_text("stale content\n")

    result_check = run_generator(src, out, ["--check"])
    assert result_check.returncode != 0, (
        f"--check should exit nonzero on drift.\nstdout: {result_check.stdout}\nstderr: {result_check.stderr}"
    )
