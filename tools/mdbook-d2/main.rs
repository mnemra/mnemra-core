//! mdbook-d2: mdBook preprocessor that renders D2 fenced code blocks to inline SVG.
//!
//! # Protocol
//!
//! When invoked as a preprocessor, mdBook writes a JSON array `[Context, Book]`
//! to this process's stdin and expects the modified Book JSON on stdout.
//!
//! When invoked as `mdbook-d2 supports <renderer>`, exits 0 for "html" and
//! non-zero for all other renderers.
//!
//! # D2 block detection
//!
//! A fenced code block whose info string starts with `d2` (first
//! whitespace-delimited token) is passed to the `d2` CLI. The block is replaced
//! with the inline SVG that `d2` emits on stdout.
//!
//! # D2 invocation
//!
//! Source is written to a temporary file. `d2` is invoked as:
//!   `d2 <tempfile> -`
//! (outputs SVG to stdout). This avoids any shell interpretation — no `sh -c`,
//! no string interpolation of user-supplied content into the command line.

use std::io::{self, Read, Write};
use std::process::{self, Command};

use serde_json::Value;
use tempfile::NamedTempFile;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // supports <renderer> subcommand
    if args.get(1).map(String::as_str) == Some("supports") {
        let renderer = args.get(2).map(String::as_str).unwrap_or("");
        if renderer == "html" {
            process::exit(0);
        } else {
            process::exit(1);
        }
    }

    // Preprocessor mode: read [Context, Book] from stdin, transform, emit Book to stdout.
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("mdbook-d2: failed to read stdin: {e}");
        process::exit(1);
    }

    let mut pair: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mdbook-d2: failed to parse JSON from stdin: {e}");
            process::exit(1);
        }
    };

    // pair is [Context, Book]; we only modify the Book (index 1).
    let book = match pair.get_mut(1) {
        Some(b) => b,
        None => {
            eprintln!("mdbook-d2: JSON input must be a [Context, Book] array");
            process::exit(1);
        }
    };

    if let Err(e) = process_book(book) {
        // e carries the exit code we should use
        eprintln!("{}", e.message);
        process::exit(e.code);
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    if let Err(e) = serde_json::to_writer(&mut out, book) {
        eprintln!("mdbook-d2: failed to serialize output: {e}");
        process::exit(1);
    }
    // mdBook expects a newline after the JSON
    if let Err(e) = out.write_all(b"\n") {
        eprintln!("mdbook-d2: failed to write output: {e}");
        process::exit(1);
    }
}

/// A structured error carrying an exit code and a message already suitable for stderr.
struct PreprocessorError {
    message: String,
    code: i32,
}

impl PreprocessorError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Walk the Book value and transform all Chapter sections in place.
fn process_book(book: &mut Value) -> Result<(), PreprocessorError> {
    let sections = match book.get_mut("sections") {
        Some(Value::Array(arr)) => arr,
        _ => return Ok(()),
    };

    for section in sections.iter_mut() {
        process_section(section)?;
    }

    Ok(())
}

/// Process a single section, which may be a Chapter (with sub_items) or a Separator.
fn process_section(section: &mut Value) -> Result<(), PreprocessorError> {
    if let Some(chapter) = section.get_mut("Chapter") {
        process_chapter(chapter)?;
    }
    // Separator and PartTitle sections have no content to transform.
    Ok(())
}

/// Process a single chapter: transform its content, then recurse into sub_items.
fn process_chapter(chapter: &mut Value) -> Result<(), PreprocessorError> {
    if let Some(Value::String(s)) = chapter.get_mut("content") {
        let transformed = transform_content(s)?;
        *s = transformed;
    }

    // Recurse into sub_items (each sub_item is a section)
    if let Some(Value::Array(sub_items)) = chapter.get_mut("sub_items") {
        for item in sub_items.iter_mut() {
            process_section(item)?;
        }
    }

    Ok(())
}

/// Scan markdown content for d2 fenced blocks. For each block found, render it
/// to SVG via the d2 CLI and splice the result in place of the fenced block.
///
/// Non-d2 fenced blocks pass through unchanged.
fn transform_content(content: &str) -> Result<String, PreprocessorError> {
    let mut output = String::with_capacity(content.len());
    let mut in_d2_fence = false;
    let mut in_other_fence = false;
    let mut d2_source = String::new();
    let mut fence_open_char = '`';
    let mut fence_open_len = 0usize;

    let lines: Vec<&str> = content.split('\n').collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if in_d2_fence {
            // Check if this line closes the d2 fence (same char, same or greater length, only whitespace after)
            if is_fence_close(line, fence_open_char, fence_open_len) {
                // Render the collected d2 source
                let svg = render_d2(&d2_source)?;
                output.push_str(&svg);
                output.push('\n');
                in_d2_fence = false;
                d2_source.clear();
            } else {
                d2_source.push_str(line);
                d2_source.push('\n');
            }
        } else if in_other_fence {
            output.push_str(line);
            output.push('\n');
            if is_fence_close(line, fence_open_char, fence_open_len) {
                in_other_fence = false;
            }
        } else {
            // Check if this is a fence opener
            if let Some((ch, len, info)) = parse_fence_open(line) {
                fence_open_char = ch;
                fence_open_len = len;
                let first_token = info.split_whitespace().next().unwrap_or("");
                if first_token == "d2" {
                    in_d2_fence = true;
                    d2_source.clear();
                    // Do not emit the fence line — the block will be replaced by SVG
                } else {
                    in_other_fence = true;
                    output.push_str(line);
                    output.push('\n');
                }
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        i += 1;
    }

    // Unclosed fence: emit whatever we collected as-is (best-effort passthrough)
    if in_d2_fence {
        // Reconstruct the original fenced block without transformation
        let fence: String = std::iter::repeat_n(fence_open_char, fence_open_len).collect();
        output.push_str(&fence);
        output.push_str("d2\n");
        output.push_str(&d2_source);
    }

    Ok(output)
}

/// Parse a CommonMark fence opener line.
///
/// Returns `(fence_char, fence_length, info_string)` if the line is a valid
/// fence opener (3+ backtick or tilde characters at the start, with optional
/// leading spaces up to 3). Returns `None` otherwise.
fn parse_fence_open(line: &str) -> Option<(char, usize, &str)> {
    // CommonMark allows up to 3 spaces of indentation before the fence
    let stripped = line.trim_start_matches(' ');
    let leading_spaces = line.len() - stripped.len();
    if leading_spaces > 3 {
        return None;
    }

    let fence_char = stripped.chars().next()?;
    if fence_char != '`' && fence_char != '~' {
        return None;
    }

    let fence_len = stripped.chars().take_while(|&c| c == fence_char).count();
    if fence_len < 3 {
        return None;
    }

    let info = stripped[fence_len..].trim_start();

    // Backtick fences may not have backticks in the info string
    if fence_char == '`' && info.contains('`') {
        return None;
    }

    Some((fence_char, fence_len, info))
}

/// Check whether `line` closes a fence that was opened with `open_char` repeated
/// `open_len` times.
///
/// A closing fence is the same character repeated at least `open_len` times,
/// followed only by optional whitespace, with up to 3 leading spaces.
fn is_fence_close(line: &str, open_char: char, open_len: usize) -> bool {
    let stripped = line.trim_start_matches(' ');
    let leading_spaces = line.len() - stripped.len();
    if leading_spaces > 3 {
        return false;
    }

    let fence_len = stripped.chars().take_while(|&c| c == open_char).count();
    if fence_len < open_len {
        return false;
    }

    stripped[fence_len..].trim().is_empty()
}

/// Render a D2 source string to SVG by shelling out to the `d2` CLI.
///
/// Source is written to a temporary file. d2 is invoked as:
///   d2 <tempfile> -
/// which reads from `<tempfile>` and writes SVG to stdout.
///
/// On success, returns the SVG string.
/// On failure (missing binary or d2 error), returns a `PreprocessorError`.
fn render_d2(source: &str) -> Result<String, PreprocessorError> {
    // Write source to a temp file to avoid any shell interpolation risk.
    let mut tmp = NamedTempFile::new().map_err(|e| {
        PreprocessorError::new(1, format!("mdbook-d2: failed to create temp file: {e}"))
    })?;
    tmp.write_all(source.as_bytes()).map_err(|e| {
        PreprocessorError::new(1, format!("mdbook-d2: failed to write to temp file: {e}"))
    })?;
    // Flush so d2 sees the full content.
    tmp.flush().map_err(|e| {
        PreprocessorError::new(1, format!("mdbook-d2: failed to flush temp file: {e}"))
    })?;

    let tmp_path = tmp.path().to_owned();

    let output = Command::new("d2")
        .arg(&tmp_path) // input file
        .arg("-")       // output: stdout
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                PreprocessorError::new(
                    1,
                    "mdbook-d2: `d2` not found on PATH. Install d2: https://d2lang.com/tour/install",
                )
            } else {
                PreprocessorError::new(1, format!("mdbook-d2: failed to spawn d2: {e}"))
            }
        })?;

    if !output.status.success() {
        // Surface d2's stderr verbatim — no enrichment.
        let stderr = io::stderr();
        let mut err = stderr.lock();
        let _ = err.write_all(&output.stderr);
        let code = output.status.code().unwrap_or(1);
        // The message is empty because we already wrote d2's stderr above.
        return Err(PreprocessorError::new(code, ""));
    }

    String::from_utf8(output.stdout).map_err(|e| {
        PreprocessorError::new(1, format!("mdbook-d2: d2 produced non-UTF-8 output: {e}"))
    })
}
