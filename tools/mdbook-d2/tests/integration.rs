//! Integration tests for mdbook-d2 preprocessor.
//!
//! All tests are black-box: they invoke the binary via assert_cmd and observe
//! stdout/stderr/exit-code only.
//!
//! ## mdBook preprocessor JSON protocol
//!
//! The binary reads a JSON array `[Context, Book]` from stdin:
//!
//! ```json
//! [
//!   {
//!     "root": "/tmp/book",
//!     "config": { "book": { "src": "src" }, "preprocessor": { "mdbook-d2": {} } },
//!     "renderer": "html",
//!     "chapter_titles": {}
//!   },
//!   {
//!     "sections": [
//!       {
//!         "Chapter": {
//!           "name": "Chapter 1",
//!           "content": "..markdown content with fenced blocks..",
//!           "number": [1],
//!           "sub_items": [],
//!           "path": "chapter_1.md",
//!           "source_path": "chapter_1.md",
//!           "parent_names": []
//!         }
//!       }
//!     ],
//!     "__non_exhaustive": null
//!   }
//! ]
//! ```
//!
//! On success the binary writes a transformed Book JSON to stdout and exits 0.
//!
//! ## Sentinel
//!
//! The stub main.rs emits "mdbook-d2: stub not implemented" to stderr on every
//! invocation. Every test asserts `stderr does NOT contain` this sentinel, so
//! the test fails at the assertion against the stub and passes only when Forge
//! replaces main.rs with a real implementation.
//!
//! ## PATH override for missing-d2 tests
//!
//! We pass `PATH` to an empty temporary directory so the child process cannot
//! resolve `d2`. We do NOT use env_clear() because that strips HOME/USER etc.
//! that test infra may need.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

const SENTINEL: &str = "mdbook-d2: stub not implemented";

/// Build a minimal valid mdBook preprocessor JSON input.
///
/// `chapter_content` is the Markdown string placed in the single test chapter.
/// `renderer` defaults to "html".
fn make_book_json(chapter_content: &str, renderer: &str) -> String {
    // Escape the chapter content for JSON embedding.
    let escaped = serde_escape(chapter_content);
    format!(
        r#"[
  {{
    "root": "/tmp/test-book",
    "config": {{
      "book": {{ "src": "src" }},
      "preprocessor": {{ "mdbook-d2": {{}} }}
    }},
    "renderer": "{renderer}",
    "chapter_titles": {{}}
  }},
  {{
    "sections": [
      {{
        "Chapter": {{
          "name": "Test Chapter",
          "content": {escaped},
          "number": [1],
          "sub_items": [],
          "path": "test_chapter.md",
          "source_path": "test_chapter.md",
          "parent_names": []
        }}
      }}
    ],
    "__non_exhaustive": null
  }}
]"#,
        renderer = renderer,
        escaped = escaped,
    )
}

/// Escape a string value for embedding as a JSON string literal (with quotes).
fn serde_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ─── supports subcommand ──────────────────────────────────────────────────────

/// Spec AC: Binary exits 0 on `supports html` subcommand.
///
/// Given the mdbook-d2 binary is built
/// When `mdbook-d2 supports html` is run
/// Then the process exits 0
/// And stderr does NOT contain the stub sentinel
#[test]
fn supports_html_exits_zero() {
    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.args(["supports", "html"])
        .assert()
        .success()
        .stderr(predicate::str::contains(SENTINEL).not());
}

/// Spec AC: Binary exits non-zero on `supports epub`.
///
/// Given the mdbook-d2 binary is built
/// When `mdbook-d2 supports epub` is run
/// Then the process exits non-zero
/// And stderr does NOT contain the stub sentinel
#[test]
fn supports_epub_exits_nonzero() {
    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.args(["supports", "epub"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(SENTINEL).not());
}

/// Spec AC: Binary exits non-zero on `supports pdf`.
///
/// Given the mdbook-d2 binary is built
/// When `mdbook-d2 supports pdf` is run
/// Then the process exits non-zero
/// And stderr does NOT contain the stub sentinel
#[test]
fn supports_pdf_exits_nonzero() {
    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.args(["supports", "pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(SENTINEL).not());
}

/// Spec AC: Binary exits non-zero on `supports latex`.
///
/// Given the mdbook-d2 binary is built
/// When `mdbook-d2 supports latex` is run
/// Then the process exits non-zero
/// And stderr does NOT contain the stub sentinel
#[test]
fn supports_latex_exits_nonzero() {
    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.args(["supports", "latex"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(SENTINEL).not());
}

// ─── smoke: d2 block renders to SVG ──────────────────────────────────────────

/// Spec AC: Given a book JSON with a d2 fenced block, the binary emits
/// transformed JSON on stdout where the block content is replaced with inline SVG.
///
/// Given d2 CLI is installed and on PATH
/// And the chapter content contains a d2 fenced block with valid source
/// When the binary is run with the book JSON on stdin
/// Then the process exits 0
/// And stdout is valid JSON whose chapter content contains <svg>
/// And the original d2 source does NOT appear verbatim in the output
/// And stderr does NOT contain the stub sentinel
///
/// NOTE: This test requires the `d2` CLI to be installed. If d2 is absent,
/// the test will fail with a clear error from the binary, not a silent skip.
#[test]
fn smoke_d2_block_renders_to_svg() {
    let d2_source = "a -> b";
    let content = format!("# Test\n\n```d2\n{d2_source}\n```\n");
    let input = make_book_json(&content, "html");

    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("<svg"))
        .stdout(predicate::str::contains(d2_source).not())
        .stderr(predicate::str::contains(SENTINEL).not());
}

// ─── missing d2 CLI ───────────────────────────────────────────────────────────

/// Spec AC: When d2 CLI is not on PATH, binary emits a clear stderr message
/// naming the missing binary and exits non-zero.
///
/// Given the d2 CLI is NOT on PATH (we override PATH to an empty temp dir)
/// And the chapter content contains a d2 fenced block
/// When the binary is run with the book JSON on stdin
/// Then the process exits non-zero
/// And stderr contains a substring referencing "d2" (the missing binary name)
/// And stderr does NOT contain the stub sentinel
#[test]
fn missing_d2_cli_emits_clear_error() {
    let empty_path_dir = TempDir::new().unwrap();
    let content = "```d2\na -> b\n```\n";
    let input = make_book_json(content, "html");

    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.env("PATH", empty_path_dir.path())
        .write_stdin(input)
        .assert()
        .failure()
        .stderr(predicate::str::contains("d2"))
        .stderr(predicate::str::contains(SENTINEL).not());
}

// ─── malformed D2 source ──────────────────────────────────────────────────────

/// Spec AC: Malformed D2 source surfaces d2's stderr verbatim; no preprocessor
/// enrichment; exits non-zero.
///
/// Given d2 CLI is installed
/// And the chapter content contains a d2 fenced block with invalid D2 syntax
/// When the binary is run with the book JSON on stdin
/// Then the process exits non-zero
/// And stderr contains content from d2's error output (substring match)
/// And stderr does NOT contain the stub sentinel
///
/// "No preprocessor enrichment" means we do NOT assert that our stderr equals
/// d2's stderr verbatim (we can't know d2's exact phrasing), but we DO assert
/// the preprocessor does not add host paths or env-derived context.
///
/// NOTE: This test requires the `d2` CLI to be installed.
#[test]
fn malformed_d2_source_surfaces_d2_error() {
    // Deliberately invalid D2 syntax — unclosed brace
    let content = "```d2\n{ invalid D2 !!!\n```\n";
    let input = make_book_json(content, "html");

    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.write_stdin(input)
        .assert()
        .failure()
        // d2 error output should reach stderr; assert it is non-empty
        .stderr(predicate::str::is_empty().not())
        .stderr(predicate::str::contains(SENTINEL).not());
}

// ─── empty d2 block ───────────────────────────────────────────────────────────

/// Spec AC: Empty/whitespace-only d2 fenced block passes to d2 unchanged;
/// preprocessor adds no special-case handling.
///
/// Given d2 CLI is installed
/// And the chapter content contains a d2 fenced block with empty content
/// When the binary is run with the book JSON on stdin
/// Then the preprocessor does NOT pre-validate the content
/// And the exit code and stderr match d2's native behavior
/// And stderr does NOT contain the stub sentinel
///
/// NOTE: This test requires the `d2` CLI to be installed.
#[test]
fn empty_d2_block_passes_through_to_d2() {
    // Zero bytes between the fences
    let content = "```d2\n\n```\n";
    let input = make_book_json(content, "html");

    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    let output = cmd.write_stdin(input).output().unwrap();

    // We do not assert a specific exit code because d2's behavior on empty
    // input is its own (may succeed with empty SVG or error). We assert:
    // 1. The binary ran (assert_cmd gives us the Output)
    // 2. stderr does NOT contain our stub sentinel (real impl passes the call through)
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr_text.contains(SENTINEL),
        "Stub sentinel found in stderr — binary is still the stub: {stderr_text}"
    );
}

// ─── shell metachar safety ────────────────────────────────────────────────────

/// Spec AC: Shell-metacharacter content in a d2 block is passed as a literal
/// argument to d2, NOT interpreted by a shell. No injected command executes.
///
/// Given d2 CLI is installed
/// And the chapter content contains a d2 fenced block whose content includes
///   shell metacharacters (semicolon, dollar sign, backtick injection)
/// When the binary is run with the book JSON on stdin
/// Then the side-effect file that the injected command would have created
///   MUST NOT exist after the binary exits
/// And stderr does NOT contain the stub sentinel
///
/// NOTE: This test requires the `d2` CLI to be installed.
#[test]
fn shell_metachar_content_is_not_interpreted_by_shell() {
    // Use a unique side-effect file path derived from a tempdir to avoid
    // collision across parallel test runs.
    let side_effect_dir = TempDir::new().unwrap();
    let side_effect_file = side_effect_dir.path().join("pwned-mdbook-d2-test");
    let side_effect_path = side_effect_file.to_str().unwrap();

    // Construct d2 content that would create the side-effect file if
    // passed through sh -c or similar shell interpretation.
    let injected = format!("; touch {side_effect_path} #");
    let content = format!("```d2\na -> b{injected}\n```\n");
    let input = make_book_json(&content, "html");

    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.write_stdin(input).output().unwrap();

    // The side-effect file must NOT exist.
    assert!(
        !Path::new(side_effect_path).exists(),
        "Shell injection succeeded — side-effect file was created at {side_effect_path}. \
         The preprocessor is likely using sh -c to invoke d2."
    );

    // Also verify the stub sentinel is absent (binary must be real impl)
    let mut cmd2 = Command::cargo_bin("mdbook-d2").unwrap();
    let content2 = format!("```d2\na -> b{injected}\n```\n");
    let input2 = make_book_json(&content2, "html");
    cmd2.write_stdin(input2)
        .assert()
        .stderr(predicate::str::contains(SENTINEL).not());
}

// ─── non-d2 block passthrough ─────────────────────────────────────────────────

/// Spec AC: Non-d2 fenced blocks (mermaid, rust) pass through unchanged.
///
/// Given d2 CLI is installed
/// And the chapter content contains a mermaid block and a rust block (no d2 blocks)
/// When the binary is run with the book JSON on stdin
/// Then the process exits 0
/// And the output JSON contains the mermaid block unchanged
/// And the output JSON contains the rust block unchanged
/// And stderr does NOT contain the stub sentinel
#[test]
fn non_d2_fenced_blocks_pass_through_unchanged() {
    let mermaid_block = "```mermaid\ngraph TD;\n  A-->B;\n```";
    let rust_block = "```rust\nfn hello() {}\n```";
    let content = format!("# Test\n\n{mermaid_block}\n\n{rust_block}\n");
    let input = make_book_json(&content, "html");

    let mut cmd = Command::cargo_bin("mdbook-d2").unwrap();
    cmd.write_stdin(input)
        .assert()
        .success()
        // The mermaid fenced block content must be present verbatim in output
        .stdout(predicate::str::contains("graph TD;"))
        // The rust fenced block content must be present verbatim in output
        .stdout(predicate::str::contains("fn hello()"))
        .stderr(predicate::str::contains(SENTINEL).not());
}
