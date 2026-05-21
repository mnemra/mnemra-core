"""
Acceptance tests (red-phase, TDD) for the mdBook frontmatter-strip preprocessor
and the human-surface wiring of the published docs site.

Spec: docs/specs/2026-05-21-mdbook-human-surface-wiring.md (locked).

One bug, two root causes pinned here:
  1. Unwired human surface — docs/book.toml has src = "src", so mdBook builds the
     agent-first docs/src/ tree instead of the EXPLAIN-translated
     docs/_published/human/ tree. (AC #6)
  2. Frontmatter rendered as page text — no strip preprocessor is configured, so
     the leading YAML `---` block renders as visible body text. (AC #2, #3, #7)

These tests are RED against parent_commit:
  - scripts/mdbook-strip-frontmatter.py does not exist yet → all unit tests
    (AC #2, #3) fail because `uv run <missing-script>` exits nonzero with no JSON
    on stdout.
  - docs/book.toml still has src = "src" and no strip preprocessor → the e2e
    wiring test (AC #6) fails because the build serves docs/src/ (agent phrase
    present, human phrase absent), and the e2e strip test (AC #7) fails because
    `primary-audience` renders into the HTML.

Black-box only: the preprocessor is driven via subprocess on stdin/stdout per the
mdBook preprocessor protocol; the e2e tests drive the real `mdbook build`. No
implementation module is imported.

Toolchain: uv (skills/python-use.md). pytest is the runner.
"""

import json
import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).parent.parent
PREPROCESSOR = REPO_ROOT / "scripts" / "mdbook-strip-frontmatter.py"
DOCS_DIR = REPO_ROOT / "docs"

# --- AC #6 wiring literals (selected by reading the two committed trees) ---------
#
# The agent-first source (docs/src/intent/mnemra-core.md) and the EXPLAIN-
# translated human page (docs/_published/human/intent/mnemra-core.md) carry the
# same content but differ in EXPLAIN phrasing. Each literal below was confirmed
# (via grep) to appear exactly once in one file and zero times in the other.
#
# HUMAN_ONLY: present in docs/_published/human/intent/mnemra-core.md (the EXPLAIN
#   pass expanded the `idea`-tier definition with this parenthetical), ABSENT from
#   docs/src/intent/mnemra-core.md. Plain prose inside a blockquote, no inline
#   markdown → renders verbatim into the HTML <blockquote> body.
HUMAN_ONLY_PHRASE = "a captured direction with no pipeline artifact yet"
#
# AGENT_ONLY: present in docs/src/intent/mnemra-core.md (the agent source phrases
#   the register-model note this way), ABSENT from
#   docs/_published/human/intent/mnemra-core.md (EXPLAIN rewrote it to
#   "The amendment is tracked separately, and it's the reason..."). Plain prose.
AGENT_ONLY_PHRASE = "tracked separately and is the"


# ---------------------------------------------------------------------------
# Preprocessor invocation helper (black box, via subprocess)
# ---------------------------------------------------------------------------

def run_preprocessor(args: list[str], stdin_text: str | None = None) -> subprocess.CompletedProcess:
    """Invoke the preprocessor through `uv run`, mirroring how mdBook + CI call it.

    At parent_commit the script does not exist, so `uv run` exits nonzero — that is
    the intended RED failure mode for the unit tests.
    """
    cmd = ["uv", "run", "--directory", str(REPO_ROOT), str(PREPROCESSOR), *args]
    return subprocess.run(
        cmd,
        input=stdin_text,
        capture_output=True,
        text=True,
    )


def make_book(items: list[dict]) -> str:
    """Build the `[context, book]` JSON array the preprocessor reads on stdin.

    Per the mdBook preprocessor protocol, the book object has key `items`; each
    item may carry a `Chapter` dict with `content` (markdown) and `sub_items`
    (a recursive list). The preprocessor must echo back the modified `book`
    object (NOT the array).
    """
    context = {"root": "/tmp/book", "renderer": "html"}
    book = {"items": items}
    return json.dumps([context, book])


def chapter(content: str, sub_items: list[dict] | None = None, name: str = "Ch") -> dict:
    return {"Chapter": {"name": name, "content": content, "sub_items": sub_items or []}}


def parse_book(stdout: str) -> dict:
    """Parse the preprocessor's stdout as the modified `book` object."""
    return json.loads(stdout)


FRONTMATTER = "---\ntitle: \"X\"\nsummary: \"s\"\nprimary-audience: agent\n---\n"


# ===========================================================================
# AC #2 — `supports` protocol
# ===========================================================================

def test_supports_html_exits_zero():
    """
    Scenario: mdBook asks whether the preprocessor supports the html renderer
      Given the strip-frontmatter preprocessor
      When it is invoked as `<cmd> supports html`
      Then it exits 0 (it supports all renderers)
    (AC #2)
    """
    result = run_preprocessor(["supports", "html"])
    assert result.returncode == 0, (
        "Expected `supports html` to exit 0.\n"
        f"exit={result.returncode}\nstdout={result.stdout!r}\nstderr={result.stderr!r}"
    )


# ===========================================================================
# AC #3 — strip behavior (unit, via stdin/stdout)
# ===========================================================================

def test_leading_frontmatter_block_is_stripped():
    """
    Scenario: a chapter whose content begins with a YAML frontmatter block
      Given a book with one Chapter whose content starts with `---\\n...\\n---\\n`
      When the preprocessor runs
      Then the leading frontmatter block is removed from that chapter's content
    (AC #3 — strip)
    """
    body = "# Real Heading\n\nReal body text.\n"
    stdin = make_book([chapter(FRONTMATTER + body)])
    result = run_preprocessor([], stdin_text=stdin)
    assert result.returncode == 0, f"preprocessor failed: stderr={result.stderr!r}"

    book = parse_book(result.stdout)
    out_content = book["items"][0]["Chapter"]["content"]
    assert "primary-audience" not in out_content, (
        f"frontmatter key leaked into stripped content: {out_content!r}"
    )
    assert "title:" not in out_content, f"frontmatter leaked: {out_content!r}"
    assert "Real body text." in out_content, f"body was lost: {out_content!r}"


def test_chapter_without_frontmatter_is_byte_unchanged():
    """
    Scenario: a chapter with no leading frontmatter
      Given a book with one Chapter whose content does NOT start with `---`
      When the preprocessor runs
      Then that chapter's content is returned byte-for-byte unchanged
    (AC #3 — no-op on non-frontmatter content)
    """
    original = "# Just a heading\n\nNo frontmatter here.\n"
    stdin = make_book([chapter(original)])
    result = run_preprocessor([], stdin_text=stdin)
    assert result.returncode == 0, f"preprocessor failed: stderr={result.stderr!r}"

    book = parse_book(result.stdout)
    out_content = book["items"][0]["Chapter"]["content"]
    assert out_content == original, (
        f"content with no frontmatter must be byte-unchanged.\n"
        f"expected={original!r}\nactual={out_content!r}"
    )


def test_nested_sub_items_frontmatter_is_stripped_recursively():
    """
    Scenario: a frontmatter block on a nested sub-chapter
      Given a book whose top Chapter has a sub_items Chapter, both carrying
            leading frontmatter
      When the preprocessor runs
      Then the frontmatter is stripped at BOTH levels (recursion)
    (AC #3 — recursion into sub_items)
    """
    top_body = "# Top\n\nTop body.\n"
    nested_body = "# Nested\n\nNested body.\n"
    nested = chapter(FRONTMATTER + nested_body, name="Nested")
    top = chapter(FRONTMATTER + top_body, sub_items=[nested], name="Top")
    stdin = make_book([top])

    result = run_preprocessor([], stdin_text=stdin)
    assert result.returncode == 0, f"preprocessor failed: stderr={result.stderr!r}"

    book = parse_book(result.stdout)
    top_out = book["items"][0]["Chapter"]
    nested_out = top_out["sub_items"][0]["Chapter"]

    assert "primary-audience" not in top_out["content"], (
        f"top-level frontmatter not stripped: {top_out['content']!r}"
    )
    assert "Top body." in top_out["content"]
    assert "primary-audience" not in nested_out["content"], (
        f"nested frontmatter not stripped (recursion failed): {nested_out['content']!r}"
    )
    assert "Nested body." in nested_out["content"]


def test_mid_document_thematic_break_is_preserved():
    """
    Scenario: a `---` thematic break lower in the page (not at the very start)
      Given a chapter whose body has a mid-document `---` horizontal rule and NO
            leading frontmatter
      When the preprocessor runs
      Then the mid-document `---` is preserved (only LEADING frontmatter strips)
    (AC #3 — only leading frontmatter is stripped, never mid-document)
    """
    original = "# Heading\n\nFirst paragraph.\n\n---\n\nSecond paragraph after a rule.\n"
    stdin = make_book([chapter(original)])
    result = run_preprocessor([], stdin_text=stdin)
    assert result.returncode == 0, f"preprocessor failed: stderr={result.stderr!r}"

    book = parse_book(result.stdout)
    out_content = book["items"][0]["Chapter"]["content"]
    assert "\n---\n" in out_content, (
        f"mid-document thematic break must be preserved: {out_content!r}"
    )
    assert "Second paragraph after a rule." in out_content
    # No leading frontmatter present → byte-unchanged is the strongest form.
    assert out_content == original, (
        f"no leading frontmatter → content must be byte-unchanged.\n"
        f"expected={original!r}\nactual={out_content!r}"
    )


# ===========================================================================
# End-to-end — build the real book
# ===========================================================================

@pytest.fixture
def built_book(tmp_path):
    """Build the real docs/ book into an absolute temp dir.

    `mdbook -d` resolves relative paths against book.toml's dir, so we pass an
    absolute path (tmp_path is absolute). The build dir is under pytest's tmp_path,
    which pytest cleans up — AC #8 (no new tracked/untracked files in the repo).
    Requires mdbook + mdbook-mermaid + mdbook-d2 on PATH (pinned in CI).
    """
    if shutil.which("mdbook") is None:
        pytest.skip("mdbook not on PATH")

    build_dir = tmp_path / "book"
    result = subprocess.run(
        ["mdbook", "build", str(DOCS_DIR), "-d", str(build_dir)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, (
        f"`mdbook build` failed (exit {result.returncode}).\n"
        f"stdout={result.stdout}\nstderr={result.stderr}"
    )
    return build_dir


def test_e2e_build_serves_human_translated_tree(built_book):
    """
    Scenario: the published site is wired to the human-translated tree
      Given a clean `mdbook build docs/`
      When intent/mnemra-core.html is rendered
      Then it contains a phrase unique to docs/_published/human/intent/mnemra-core.md
       And it does NOT contain a phrase unique to docs/src/intent/mnemra-core.md
    This proves the build reads _published/human/, not docs/src/.
    (AC #6 — wiring)
    """
    page = built_book / "intent" / "mnemra-core.html"
    assert page.exists(), f"expected rendered page at {page}"
    html = page.read_text()

    assert HUMAN_ONLY_PHRASE in html, (
        "human-translated phrase missing — build is not serving _published/human/.\n"
        f"phrase={HUMAN_ONLY_PHRASE!r}"
    )
    assert AGENT_ONLY_PHRASE not in html, (
        "agent-source phrase present — build is still serving docs/src/.\n"
        f"phrase={AGENT_ONLY_PHRASE!r}"
    )


def test_e2e_no_frontmatter_renders_in_any_page(built_book):
    """
    Scenario: YAML frontmatter no longer renders as page text
      Given a clean `mdbook build docs/`
      When every rendered *.html page is scanned
      Then none contains the literal frontmatter key text `primary-audience`
    This proves the strip preprocessor runs at build time.
    (AC #7 — strip)
    """
    offenders = [
        str(p.relative_to(built_book))
        for p in built_book.rglob("*.html")
        if "primary-audience" in p.read_text()
    ]
    assert not offenders, (
        "frontmatter key `primary-audience` rendered into HTML pages "
        f"(strip preprocessor not active): {offenders}"
    )
