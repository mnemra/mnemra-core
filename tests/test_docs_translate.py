"""
Tests for scripts/docs-translate.py — translation pipeline.

Red-phase: all tests fail (script does not exist yet). Failure mode is
FileNotFoundError or non-zero exit from subprocess — NOT collection errors.

Test structure mirrors test_docs_llms.py: helpers at top, one scenario per
function (or parametrize when only data differs, assertions are identical).

Spec: docs/specs/2026-05-21-translation-pipeline.md
"""

import hashlib
import json
import os
import subprocess
import textwrap
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).parent.parent
GENERATOR = REPO_ROOT / "scripts" / "docs-translate.py"


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

def run_translate(
    src: Path,
    out: Path,
    prompts: Path,
    mode: str,
    extra: list[str] | None = None,
) -> subprocess.CompletedProcess:
    """Invoke docs-translate.py via uv.

    Always passes explicit --src, --out, --prompts (CWD-independent).
    mode must be one of: --plan, --finalize, --check
    """
    args = [
        "uv",
        "run",
        str(GENERATOR),
        mode,
        "--src", str(src),
        "--out", str(out),
        "--prompts", str(prompts),
    ]
    if extra:
        args.extend(extra)
    return subprocess.run(args, capture_output=True, text=True)


# ---------------------------------------------------------------------------
# Fixture builders
# ---------------------------------------------------------------------------

def sha256_of(content: str) -> str:
    """Return the SHA-256 hex of a UTF-8 string (matches hashlib.sha256 on bytes)."""
    return hashlib.sha256(content.encode()).hexdigest()


def sha256_of_path(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make_page(path: Path, *, title: str, audience: str, body: str = "Body text.") -> None:
    """Write a source page with required frontmatter."""
    os.makedirs(path.parent, exist_ok=True)
    content = textwrap.dedent(f"""\
        ---
        title: "{title}"
        summary: "Summary of {title}."
        primary-audience: {audience}
        ---

        {body}
    """)
    path.write_text(content)


def make_summary(src_dir: Path, entries: list[str]) -> None:
    """Write a minimal SUMMARY.md inside src_dir linking the given relative paths."""
    os.makedirs(src_dir, exist_ok=True)
    lines = ["# Summary", ""]
    for entry in entries:
        name = Path(entry).stem
        lines.append(f"- [{name}]({entry})")
    (src_dir / "SUMMARY.md").write_text("\n".join(lines) + "\n")


def make_glossary(path: Path, body: str = "Glossary body content.") -> None:
    """Write a glossary page (human-primary, with frontmatter that the script strips)."""
    os.makedirs(path.parent, exist_ok=True)
    content = textwrap.dedent(f"""\
        ---
        title: Glossary
        summary: "Glossary of terms."
        primary-audience: human
        ---

        {body}
    """)
    path.write_text(content)


def make_prompt(path: Path, body: str | None = None) -> None:
    """Write a minimal prompt file containing both required tokens."""
    os.makedirs(path.parent, exist_ok=True)
    if body is None:
        body = textwrap.dedent("""\
            <role>Translator role.</role>
            <task>Translate the page.</task>
            <glossary>
            {{GLOSSARY}}
            </glossary>
            <source-page>
            {{PAGE}}
            </source-page>
        """)
    path.write_text(body)


def make_manifest(path: Path, entries: dict, schema_version: int = 1) -> None:
    """Write a .translation-manifest.json to path."""
    os.makedirs(path.parent, exist_ok=True)
    manifest = {"schema_version": schema_version, "entries": entries}
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def make_valid_entry(
    *,
    source_sha256: str | None = None,
    prompt_path: str = "explain-pass.md",
    prompt_sha256: str | None = None,
    primary_audience: str = "agent",
    translated_at: str = "2026-05-21T12:00:00Z",
) -> dict:
    """Return a syntactically valid manifest entry dict."""
    return {
        "primary_audience": primary_audience,
        "prompt_path": prompt_path,
        "prompt_sha256": prompt_sha256 or ("a" * 64),
        "source_sha256": source_sha256 or ("b" * 64),
        "translated_at": translated_at,
    }


def seed_published(out: Path, rel_path: str, content: str = "translated content\n") -> Path:
    """Write a pre-existing published file (simulates a prior translation)."""
    target = out / rel_path
    os.makedirs(target.parent, exist_ok=True)
    target.write_text(content)
    return target


def minimal_fixture(tmp_path: Path) -> tuple[Path, Path, Path]:
    """
    Build a minimal fixture tree:
      src/: SUMMARY.md + page.md (agent) + glossary.md
      out/: (empty, no manifest yet)
      prompts/: explain-pass.md + strip-pass.md

    Returns (src, out, prompts).
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Test Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    return src, out, prompts


# ---------------------------------------------------------------------------
# §1 Manifest schema validation
# ---------------------------------------------------------------------------

def test_manifest_valid_round_trips_through_check(tmp_path):
    """
    Given a well-formed manifest where source, prompt hashes match, and
    published files are present.
    When --check is run.
    Then exit 0 (manifest is consistent).
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Seed published files for both sides
    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    page_sha = sha256_of_path(src / "page.md")
    prompt_sha = sha256_of_path(prompts / "explain-pass.md")

    manifest_path = out / ".translation-manifest.json"
    make_manifest(manifest_path, {
        "page.md": make_valid_entry(
            source_sha256=page_sha,
            prompt_path="explain-pass.md",
            prompt_sha256=prompt_sha,
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode == 0, (
        f"Expected exit 0 for valid manifest, got {result.returncode}.\n"
        f"stderr: {result.stderr}\nstdout: {result.stdout}"
    )


@pytest.mark.parametrize("bad_manifest,expected_message_fragment", [
    # schema_version: 2 → exit 1
    (
        {"schema_version": 2, "entries": {}},
        "schema_version",
    ),
    # Missing required field 'translated_at' → exit 1
    (
        {
            "schema_version": 1,
            "entries": {
                "page.md": {
                    "primary_audience": "agent",
                    "prompt_path": "explain-pass.md",
                    "prompt_sha256": "a" * 64,
                    "source_sha256": "b" * 64,
                    # translated_at MISSING
                }
            },
        },
        None,  # just check exit code
    ),
    # Malformed source_sha256 (not 64 hex chars) → exit 1
    (
        {
            "schema_version": 1,
            "entries": {
                "page.md": {
                    "primary_audience": "agent",
                    "prompt_path": "explain-pass.md",
                    "prompt_sha256": "a" * 64,
                    "source_sha256": "tooshort",
                    "translated_at": "2026-05-21T12:00:00Z",
                }
            },
        },
        None,
    ),
    # primary_audience: "other" → exit 1
    (
        {
            "schema_version": 1,
            "entries": {
                "page.md": {
                    "primary_audience": "other",
                    "prompt_path": "explain-pass.md",
                    "prompt_sha256": "a" * 64,
                    "source_sha256": "b" * 64,
                    "translated_at": "2026-05-21T12:00:00Z",
                }
            },
        },
        None,
    ),
    # primary_audience: "agent" paired with strip-pass.md → exit 1
    (
        {
            "schema_version": 1,
            "entries": {
                "page.md": {
                    "primary_audience": "agent",
                    "prompt_path": "strip-pass.md",  # disagrees with agent→explain
                    "prompt_sha256": "a" * 64,
                    "source_sha256": "b" * 64,
                    "translated_at": "2026-05-21T12:00:00Z",
                }
            },
        },
        None,
    ),
])
def test_manifest_schema_validation_rejects_invalid(tmp_path, bad_manifest, expected_message_fragment):
    """
    Given a manifest that violates the schema.
    When --check is run.
    Then exit 1.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    manifest_path = out / ".translation-manifest.json"
    manifest_path.write_text(json.dumps(bad_manifest, indent=2) + "\n")

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, (
        f"Expected non-zero exit for invalid manifest, got 0.\n"
        f"stderr: {result.stderr}\nstdout: {result.stdout}"
    )
    if expected_message_fragment:
        combined = result.stderr + result.stdout
        assert expected_message_fragment in combined, (
            f"Expected message fragment '{expected_message_fragment}' in output.\n"
            f"stderr: {result.stderr}\nstdout: {result.stdout}"
        )


# ---------------------------------------------------------------------------
# §2 Hash-gated logic via --plan
# ---------------------------------------------------------------------------

def test_plan_skips_page_when_hashes_match_and_files_present(tmp_path):
    """
    Given a manifest entry where source_sha256 + prompt_sha256 match reality
      AND both _published/{human,agent}/page.md exist.
    When --plan is run.
    Then the page is NOT in the plan items list.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Seed both published sides
    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    page_sha = sha256_of_path(src / "page.md")
    prompt_sha = sha256_of_path(prompts / "explain-pass.md")

    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256=page_sha,
            prompt_path="explain-pass.md",
            prompt_sha256=prompt_sha,
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    plan = json.loads(result.stdout)
    assert plan["items"] == [], (
        f"Expected empty plan items when hashes match, got: {plan['items']}"
    )


def test_plan_includes_page_when_source_sha256_mismatches(tmp_path):
    """
    Given a manifest entry where source_sha256 does NOT match the current file.
    When --plan is run.
    Then the page IS in plan items, with correct output_path and assembled_prompt.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    # Use a stale source hash (all-zeros — definitely wrong)
    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256="0" * 64,  # stale
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    plan = json.loads(result.stdout)
    assert len(plan["items"]) == 1, (
        f"Expected one plan item for stale source, got {len(plan['items'])}: {plan['items']}"
    )
    item = plan["items"][0]
    # Agent-primary → human side needs translation via explain-pass
    assert "human" in item["output_path"], (
        f"Expected output_path in human/ for agent-primary page, got: {item['output_path']}"
    )
    assert item["assembled_prompt"], "assembled_prompt must be non-empty"
    assert "{{GLOSSARY}}" not in item["assembled_prompt"], (
        "assembled_prompt must have {{GLOSSARY}} substituted"
    )
    assert "{{PAGE}}" not in item["assembled_prompt"], (
        "assembled_prompt must have {{PAGE}} substituted"
    )


def test_plan_includes_page_when_prompt_sha256_mismatches(tmp_path):
    """
    Given a manifest entry where prompt_sha256 does NOT match the current prompt file.
    When --plan is run.
    Then the page IS in plan items.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    # Use stale prompt hash
    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "page.md"),
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,  # stale prompt hash
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    plan = json.loads(result.stdout)
    assert len(plan["items"]) >= 1, (
        f"Expected plan item for stale prompt hash, got {len(plan['items'])}"
    )
    paths = [item["output_path"] for item in plan["items"]]
    assert any("page.md" in p for p in paths), (
        f"Expected page.md in plan items, got: {paths}"
    )


def test_plan_includes_page_when_published_file_missing(tmp_path):
    """
    Given hashes match but _published/{human,agent}/page.md is absent.
    When --plan is run.
    Then the page IS in plan items.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Seed agent side only; human side is missing
    seed_published(out, "agent/page.md")
    # out/human/page.md intentionally absent

    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "page.md"),
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    plan = json.loads(result.stdout)
    assert len(plan["items"]) >= 1, (
        f"Expected plan item when published file missing, got {len(plan['items'])}"
    )


# ---------------------------------------------------------------------------
# §3 Audience routing
# ---------------------------------------------------------------------------

def test_audience_routing_agent_primary_verbatim_copy_to_agent_side(tmp_path):
    """
    Given a page with primary-audience: agent.
    When --plan is run.
    Then the verbatim copy goes to _published/agent/<path>,
      and the plan entry targets _published/human/<path> with explain-pass.md.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Agent Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # No manifest → everything is stale
    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    # Verbatim copy placed on agent side
    agent_copy = out / "agent" / "page.md"
    assert agent_copy.exists(), f"Verbatim copy missing at {agent_copy}"
    assert agent_copy.read_text() == (src / "page.md").read_text(), (
        "Agent-side verbatim copy should be byte-identical to source"
    )

    # Plan entry targets human side with explain-pass
    plan = json.loads(result.stdout)
    items = plan["items"]
    assert len(items) == 1, f"Expected 1 plan item, got {len(items)}: {items}"
    item = items[0]
    assert "human" in item["output_path"], (
        f"Agent-primary page: output_path should be under human/, got: {item['output_path']}"
    )
    assert "explain-pass.md" in item["prompt_path"], (
        f"Agent-primary page: prompt_path should be explain-pass.md, got: {item['prompt_path']}"
    )


def test_audience_routing_human_primary_verbatim_copy_to_human_side(tmp_path):
    """
    Given a page with primary-audience: human.
    When --plan is run.
    Then the verbatim copy goes to _published/human/<path>,
      and the plan entry targets _published/agent/<path> with strip-pass.md.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Human Page", audience="human")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    # Verbatim copy placed on human side
    human_copy = out / "human" / "page.md"
    assert human_copy.exists(), f"Verbatim copy missing at {human_copy}"
    assert human_copy.read_text() == (src / "page.md").read_text(), (
        "Human-side verbatim copy should be byte-identical to source"
    )

    # Plan entry targets agent side with strip-pass
    plan = json.loads(result.stdout)
    items = plan["items"]
    assert len(items) == 1, f"Expected 1 plan item, got {len(items)}: {items}"
    item = items[0]
    assert "agent" in item["output_path"], (
        f"Human-primary page: output_path should be under agent/, got: {item['output_path']}"
    )
    assert "strip-pass.md" in item["prompt_path"], (
        f"Human-primary page: prompt_path should be strip-pass.md, got: {item['prompt_path']}"
    )


def test_audience_routing_missing_primary_audience_exits_1(tmp_path):
    """
    Given a page with no primary-audience frontmatter field.
    When --plan is run.
    Then exit 1 and stderr names the page.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Write page WITHOUT primary-audience field
    os.makedirs((src / "page.md").parent, exist_ok=True)
    (src / "page.md").write_text(textwrap.dedent("""\
        ---
        title: "No Audience Page"
        summary: "Missing audience field."
        ---

        Body text.
    """))

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode != 0, (
        f"Expected non-zero exit when primary-audience missing, got 0.\n"
        f"stderr: {result.stderr}"
    )
    assert "page.md" in result.stderr, (
        f"Expected page.md named in stderr, got: {result.stderr}"
    )


# ---------------------------------------------------------------------------
# §4 Prompt substitution
# ---------------------------------------------------------------------------

def test_prompt_substitution_glossary_token_replaced(tmp_path):
    """
    Given a prompt with {{GLOSSARY}} and a glossary file with known content.
    When --plan is run.
    Then assembled_prompt contains glossary body text (frontmatter stripped)
      and does NOT contain the literal {{GLOSSARY}} token.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    glossary_body = "UNIQUE_GLOSSARY_MARKER_XYZ789"
    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md", body=glossary_body)
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    plan = json.loads(result.stdout)
    assert len(plan["items"]) >= 1, "Expected at least one plan item"
    assembled = plan["items"][0]["assembled_prompt"]

    assert glossary_body in assembled, (
        f"Expected glossary body text in assembled_prompt. Got:\n{assembled[:500]}"
    )
    assert "{{GLOSSARY}}" not in assembled, (
        "{{GLOSSARY}} token must be replaced in assembled_prompt"
    )
    # Frontmatter from glossary should be stripped
    assert "primary-audience: human" not in assembled or glossary_body in assembled, (
        "Glossary frontmatter should be stripped from assembled_prompt"
    )


def test_prompt_substitution_page_token_replaced_with_frontmatter(tmp_path):
    """
    Given a page with frontmatter.
    When --plan is run.
    Then assembled_prompt contains page content including frontmatter
      and does NOT contain the literal {{PAGE}} token.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    page_body = "UNIQUE_PAGE_BODY_MARKER_ABC123"
    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Special Page Title", audience="agent", body=page_body)
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    plan = json.loads(result.stdout)
    assert len(plan["items"]) >= 1, "Expected at least one plan item"
    assembled = plan["items"][0]["assembled_prompt"]

    assert page_body in assembled, (
        f"Expected page body text in assembled_prompt. Got:\n{assembled[:500]}"
    )
    # Frontmatter is included for page (per spec: "page content (frontmatter included)")
    assert "Special Page Title" in assembled, (
        "Page frontmatter (title) should be present in assembled_prompt"
    )
    assert "{{PAGE}}" not in assembled, (
        "{{PAGE}} token must be replaced in assembled_prompt"
    )


def test_prompt_missing_glossary_token_exits_1(tmp_path):
    """
    Given a prompt file that does NOT contain {{GLOSSARY}}.
    When --plan is run.
    Then exit 1 naming the prompt file.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")

    # Prompt missing {{GLOSSARY}} token
    make_prompt(prompts / "explain-pass.md", body="<task>Translate.</task>\n{{PAGE}}\n")
    make_prompt(prompts / "strip-pass.md")

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode != 0, (
        f"Expected non-zero exit for prompt missing {{GLOSSARY}}, got 0.\nstderr: {result.stderr}"
    )
    assert "explain-pass.md" in result.stderr or "explain-pass.md" in result.stdout, (
        f"Expected prompt filename named in output.\nstderr: {result.stderr}\nstdout: {result.stdout}"
    )


def test_prompt_missing_page_token_exits_1(tmp_path):
    """
    Given a prompt file that does NOT contain {{PAGE}}.
    When --plan is run.
    Then exit 1 naming the prompt file.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")

    # Prompt missing {{PAGE}} token
    make_prompt(prompts / "explain-pass.md", body="<task>Translate.</task>\n{{GLOSSARY}}\n")
    make_prompt(prompts / "strip-pass.md")

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode != 0, (
        f"Expected non-zero exit for prompt missing {{PAGE}}, got 0.\nstderr: {result.stderr}"
    )
    assert "explain-pass.md" in result.stderr or "explain-pass.md" in result.stdout, (
        f"Expected prompt filename named in output.\nstderr: {result.stderr}\nstdout: {result.stdout}"
    )


# ---------------------------------------------------------------------------
# §5 --check mode
# ---------------------------------------------------------------------------

def test_check_clean_tree_exits_0(tmp_path):
    """
    Given a manifest where all hashes match reality and all published files exist.
    When --check is run.
    Then exit 0.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "page.md"),
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode == 0, (
        f"Expected exit 0 for clean tree, got {result.returncode}.\nstderr: {result.stderr}"
    )


def test_check_stale_source_exits_1_and_names_page(tmp_path):
    """
    Given a manifest entry where source_sha256 is stale (source edited since last manifest).
    When --check is run.
    Then exit 1 and stderr names the stale page.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    # Manifest has stale source hash
    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256="0" * 64,  # stale
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, (
        f"Expected non-zero exit for stale source, got 0.\nstdout: {result.stdout}"
    )
    assert "page.md" in result.stderr, (
        f"Expected stale page named in stderr.\nstderr: {result.stderr}"
    )


def test_check_stale_prompt_exits_1_and_names_prompt(tmp_path):
    """
    Given a manifest entry where prompt_sha256 is stale (prompt edited since last manifest).
    When --check is run.
    Then exit 1 and stderr names the prompt.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    # Manifest has stale prompt hash
    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "page.md"),
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,  # stale prompt
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, (
        f"Expected non-zero exit for stale prompt, got 0.\nstdout: {result.stdout}"
    )
    assert "explain-pass.md" in result.stderr, (
        f"Expected prompt name in stderr.\nstderr: {result.stderr}"
    )


def test_check_missing_published_file_exits_1(tmp_path):
    """
    Given a manifest entry but the published file at _published/<dir>/<path> is absent.
    When --check is run.
    Then exit 1.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Only seed agent side; human side absent
    seed_published(out, "agent/page.md")
    # human/page.md intentionally absent

    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "page.md"),
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        )
    })

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, (
        f"Expected non-zero exit for missing published file, got 0.\nstdout: {result.stdout}"
    )


def test_check_does_not_mutate_filesystem(tmp_path):
    """
    Given any drift state.
    When --check is run.
    Then no files are copied, no orphans removed, filesystem is byte-stable.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Stale manifest → drift state
    make_manifest(out / ".translation-manifest.json", {
        "page.md": make_valid_entry(
            source_sha256="0" * 64,
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,
            primary_audience="agent",
        )
    })

    # Snapshot tree before --check
    def tree_snapshot(root: Path) -> dict[str, bytes]:
        return {
            str(p.relative_to(root)): p.read_bytes()
            for p in root.rglob("*")
            if p.is_file()
        }

    before = tree_snapshot(out)

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, "Expected non-zero exit for drift"

    after = tree_snapshot(out)
    assert before == after, (
        f"--check must not mutate the filesystem.\n"
        f"Before: {sorted(before.keys())}\nAfter: {sorted(after.keys())}"
    )


# ---------------------------------------------------------------------------
# §6 --finalize mode
# ---------------------------------------------------------------------------

def _setup_finalize_fixture(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path]:
    """
    Build a complete fixture for --finalize tests:
    - src tree with one page
    - prompts
    - pending sidecar listing one output to finalize
    - Returns (src, out, prompts, manifest_path, pending_path)
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Seed the verbatim copy (agent side, done by --plan)
    seed_published(out, "agent/page.md")

    # The human-side translation is the output --finalize must validate
    human_output = out / "human" / "page.md"

    manifest_path = out / ".translation-manifest.json"
    pending_path = out / ".translation-pending.json"

    # Write a pending sidecar describing the expected output
    pending = {
        "schema_version": 1,
        "items": [
            {
                "source_path": str(src / "page.md"),
                "output_path": str(human_output),
                "primary_audience": "agent",
                "prompt_path": str(prompts / "explain-pass.md"),
                "source_sha256": sha256_of_path(src / "page.md"),
                "prompt_sha256": sha256_of_path(prompts / "explain-pass.md"),
            }
        ],
        "orphans_to_remove": [],
    }
    pending_path.write_text(json.dumps(pending, indent=2) + "\n")

    # Manifest starts empty (no prior entry for page.md)
    make_manifest(manifest_path, {})

    return src, out, prompts, manifest_path, pending_path


def test_finalize_success_updates_manifest_and_removes_sidecar(tmp_path):
    """
    Given all planned outputs are present and non-empty.
    When --finalize is run.
    Then manifest is updated, pending sidecar is removed, exit 0.
    """
    src, out, prompts, manifest_path, pending_path = _setup_finalize_fixture(tmp_path)

    # Write the translation output (non-empty)
    os.makedirs((out / "human" / "page.md").parent, exist_ok=True)
    (out / "human" / "page.md").write_text("# Translated\n\nTranslated content.\n")

    result = run_translate(src, out, prompts, "--finalize")
    assert result.returncode == 0, (
        f"Expected exit 0 on finalize success, got {result.returncode}.\n"
        f"stderr: {result.stderr}\nstdout: {result.stdout}"
    )

    # Manifest updated with new entry
    manifest = json.loads(manifest_path.read_text())
    assert "page.md" in manifest["entries"], (
        f"Expected page.md entry in manifest after finalize. Entries: {manifest['entries']}"
    )

    # Pending sidecar removed
    assert not pending_path.exists(), "Pending sidecar must be removed on successful finalize"


def test_finalize_missing_output_exits_1_leaves_manifest_and_sidecar_unchanged(tmp_path):
    """
    Given a planned output that is missing from disk.
    When --finalize is run.
    Then exit 1, manifest unchanged, pending sidecar preserved.
    """
    src, out, prompts, manifest_path, pending_path = _setup_finalize_fixture(tmp_path)

    # Do NOT write the translation output — it's missing
    manifest_before = manifest_path.read_bytes()
    pending_before = pending_path.read_bytes()

    result = run_translate(src, out, prompts, "--finalize")
    assert result.returncode != 0, (
        f"Expected non-zero exit for missing output, got 0.\nstdout: {result.stdout}"
    )

    # Manifest unchanged
    assert manifest_path.read_bytes() == manifest_before, (
        "Manifest must be unchanged when finalize fails (missing output)"
    )
    # Pending sidecar preserved
    assert pending_path.exists(), "Pending sidecar must be preserved when finalize fails"
    assert pending_path.read_bytes() == pending_before, (
        "Pending sidecar must be byte-identical when finalize fails"
    )


def test_finalize_empty_output_exits_1_leaves_manifest_unchanged(tmp_path):
    """
    Given a planned output that is present but has zero bytes.
    When --finalize is run.
    Then exit 1, manifest unchanged.
    """
    src, out, prompts, manifest_path, pending_path = _setup_finalize_fixture(tmp_path)

    # Write empty translation output
    os.makedirs((out / "human" / "page.md").parent, exist_ok=True)
    (out / "human" / "page.md").write_bytes(b"")  # zero bytes

    manifest_before = manifest_path.read_bytes()

    result = run_translate(src, out, prompts, "--finalize")
    assert result.returncode != 0, (
        f"Expected non-zero exit for empty output, got 0.\nstdout: {result.stdout}"
    )
    assert manifest_path.read_bytes() == manifest_before, (
        "Manifest must be unchanged when finalize fails (empty output)"
    )


def test_finalize_removes_orphan_manifest_entries(tmp_path):
    """
    Given a pending sidecar that lists orphan manifest entries to remove.
    When --finalize is run successfully.
    Then orphan entries are deleted from the manifest in the same write transaction.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")

    # Pre-existing manifest with page.md entry (fresh) + deleted.md entry (orphan)
    page_sha = sha256_of_path(src / "page.md")
    prompt_sha = sha256_of_path(prompts / "explain-pass.md")
    manifest_path = out / ".translation-manifest.json"
    make_manifest(manifest_path, {
        "page.md": make_valid_entry(
            source_sha256=page_sha,
            prompt_path="explain-pass.md",
            prompt_sha256=prompt_sha,
            primary_audience="agent",
        ),
        "deleted.md": make_valid_entry(  # orphan — source was deleted
            source_sha256="d" * 64,
            prompt_path="explain-pass.md",
            prompt_sha256=prompt_sha,
            primary_audience="agent",
        ),
    })

    human_output = out / "human" / "page.md"
    os.makedirs(human_output.parent, exist_ok=True)
    human_output.write_text("# Translated\n\nContent.\n")

    # Pending sidecar: one new item + one orphan to remove
    pending_path = out / ".translation-pending.json"
    pending = {
        "schema_version": 1,
        "items": [
            {
                "source_path": str(src / "page.md"),
                "output_path": str(human_output),
                "primary_audience": "agent",
                "prompt_path": str(prompts / "explain-pass.md"),
                "source_sha256": page_sha,
                "prompt_sha256": prompt_sha,
            }
        ],
        "orphans_to_remove": ["deleted.md"],
    }
    pending_path.write_text(json.dumps(pending, indent=2) + "\n")

    result = run_translate(src, out, prompts, "--finalize")
    assert result.returncode == 0, (
        f"Expected finalize success, got {result.returncode}.\nstderr: {result.stderr}"
    )

    manifest = json.loads(manifest_path.read_text())
    assert "deleted.md" not in manifest["entries"], (
        "Orphan manifest entry 'deleted.md' must be deleted by --finalize"
    )
    assert "page.md" in manifest["entries"], (
        "Non-orphan entry 'page.md' must remain in manifest"
    )


# ---------------------------------------------------------------------------
# §7 Manifest single-writer invariant
# ---------------------------------------------------------------------------

def test_plan_does_not_mutate_manifest(tmp_path):
    """
    Given stale source + existing manifest.
    When --plan is run.
    Then manifest file is byte-identical before and after.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    manifest_path = out / ".translation-manifest.json"
    make_manifest(manifest_path, {
        "page.md": make_valid_entry(
            source_sha256="0" * 64,  # stale → plan will include this page
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,
            primary_audience="agent",
        )
    })

    manifest_before = manifest_path.read_bytes()

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    # Confirm something is in the plan (ensures the stale case was exercised)
    plan = json.loads(result.stdout)
    assert len(plan["items"]) >= 1, "Plan should have items for stale source"

    # Manifest must be byte-identical
    assert manifest_path.read_bytes() == manifest_before, (
        "--plan must NOT mutate the manifest (single-writer invariant: only --finalize writes it)"
    )


def test_check_does_not_mutate_manifest(tmp_path):
    """
    Given drift state with existing manifest.
    When --check is run.
    Then manifest file is byte-identical before and after.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    make_summary(src, ["page.md", "glossary.md"])
    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/page.md")
    seed_published(out, "human/page.md")

    manifest_path = out / ".translation-manifest.json"
    make_manifest(manifest_path, {
        "page.md": make_valid_entry(
            source_sha256="0" * 64,  # stale → --check will report drift
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,
            primary_audience="agent",
        )
    })

    manifest_before = manifest_path.read_bytes()

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, "Expected non-zero exit for drift"

    assert manifest_path.read_bytes() == manifest_before, (
        "--check must NOT mutate the manifest (single-writer invariant)"
    )


# ---------------------------------------------------------------------------
# §8 SUMMARY.md special case
# ---------------------------------------------------------------------------

def test_summary_md_copied_to_both_sides_not_in_plan(tmp_path):
    """
    Given a SUMMARY.md in docs/src/.
    When --plan is run.
    Then SUMMARY.md is copied verbatim to both _published/human/ and _published/agent/,
      and it does NOT appear in plan items or the manifest.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    summary_content = "# Summary\n\n- [Page](page.md)\n- [Glossary](glossary.md)\n"
    make_summary(src, ["page.md", "glossary.md"])
    (src / "SUMMARY.md").write_text(summary_content)  # overwrite with known content

    make_page(src / "page.md", title="Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    # SUMMARY.md copied to both sides
    human_summary = out / "human" / "SUMMARY.md"
    agent_summary = out / "agent" / "SUMMARY.md"
    assert human_summary.exists(), f"SUMMARY.md must be copied to human side: {human_summary}"
    assert agent_summary.exists(), f"SUMMARY.md must be copied to agent side: {agent_summary}"
    assert human_summary.read_text() == summary_content, "Human SUMMARY.md must be verbatim copy"
    assert agent_summary.read_text() == summary_content, "Agent SUMMARY.md must be verbatim copy"

    # SUMMARY.md NOT in plan items
    plan = json.loads(result.stdout)
    plan_paths = [item["source_path"] for item in plan["items"]]
    assert not any("SUMMARY.md" in p for p in plan_paths), (
        f"SUMMARY.md must not appear in plan items. Got: {plan_paths}"
    )


# ---------------------------------------------------------------------------
# §9 Orphan cleanup
# ---------------------------------------------------------------------------

def test_plan_removes_orphan_published_files_and_writes_pending_sidecar(tmp_path):
    """
    Given a source page was deleted (orphan), with prior published files on both sides.
    When --plan is run.
    Then _published/{human,agent}/<path> files are removed,
      the orphan is listed in the pending sidecar,
      and the manifest is byte-identical before and after.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    # Source tree has only active.md; orphan.md was deleted
    make_summary(src, ["active.md", "glossary.md"])
    make_page(src / "active.md", title="Active Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    # Published files for both active and orphan
    seed_published(out, "agent/active.md")
    seed_published(out, "human/active.md")
    orphan_agent = seed_published(out, "agent/orphan.md")
    orphan_human = seed_published(out, "human/orphan.md")

    # Manifest includes orphan.md (source was deleted after this was written)
    manifest_path = out / ".translation-manifest.json"
    make_manifest(manifest_path, {
        "active.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "active.md"),
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        ),
        "orphan.md": make_valid_entry(  # source deleted
            source_sha256="0" * 64,
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,
            primary_audience="agent",
        ),
    })

    manifest_before = manifest_path.read_bytes()

    result = run_translate(src, out, prompts, "--plan")
    assert result.returncode == 0, f"--plan failed:\nstderr: {result.stderr}"

    # Orphan published files removed by --plan
    assert not orphan_agent.exists(), f"Orphan agent file should be removed: {orphan_agent}"
    assert not orphan_human.exists(), f"Orphan human file should be removed: {orphan_human}"

    # Pending sidecar exists and mentions the orphan
    pending_path = out / ".translation-pending.json"
    assert pending_path.exists(), "Pending sidecar must be written by --plan"
    pending = json.loads(pending_path.read_text())
    assert "orphan.md" in str(pending), (
        f"Pending sidecar must list orphan.md. Got:\n{pending}"
    )

    # Manifest unchanged by --plan
    assert manifest_path.read_bytes() == manifest_before, (
        "--plan must NOT mutate the manifest; orphan entry removal is deferred to --finalize"
    )


def test_check_reports_orphans_as_drift_without_removing_them(tmp_path):
    """
    Given an orphan entry in the manifest (source page deleted).
    When --check is run.
    Then exit 1 (drift reported), but orphan published files are NOT removed,
      and manifest is NOT mutated.
    """
    src = tmp_path / "src"
    out = tmp_path / "out"
    prompts = tmp_path / "prompts"
    out.mkdir()

    # Source tree: only active.md (orphan.md source was deleted)
    make_summary(src, ["active.md", "glossary.md"])
    make_page(src / "active.md", title="Active Page", audience="agent")
    make_glossary(src / "glossary.md")
    make_prompt(prompts / "explain-pass.md")
    make_prompt(prompts / "strip-pass.md")

    seed_published(out, "agent/active.md")
    seed_published(out, "human/active.md")
    orphan_agent = seed_published(out, "agent/orphan.md")
    orphan_human = seed_published(out, "human/orphan.md")

    manifest_path = out / ".translation-manifest.json"
    make_manifest(manifest_path, {
        "active.md": make_valid_entry(
            source_sha256=sha256_of_path(src / "active.md"),
            prompt_path="explain-pass.md",
            prompt_sha256=sha256_of_path(prompts / "explain-pass.md"),
            primary_audience="agent",
        ),
        "orphan.md": make_valid_entry(
            source_sha256="0" * 64,
            prompt_path="explain-pass.md",
            prompt_sha256="0" * 64,
            primary_audience="agent",
        ),
    })

    manifest_before = manifest_path.read_bytes()

    result = run_translate(src, out, prompts, "--check")
    assert result.returncode != 0, (
        f"Expected exit 1 for orphan drift, got 0.\nstdout: {result.stdout}"
    )

    # Orphan files NOT removed by --check
    assert orphan_agent.exists(), "--check must not remove orphan published files"
    assert orphan_human.exists(), "--check must not remove orphan published files"

    # Manifest unchanged
    assert manifest_path.read_bytes() == manifest_before, (
        "--check must NOT mutate the manifest"
    )
