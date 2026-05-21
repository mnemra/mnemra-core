#!/usr/bin/env python3
"""mdBook preprocessor: strip a leading YAML frontmatter block from each chapter.

Pages in docs/_published/human/ carry YAML frontmatter (title / summary /
primary-audience). mdBook has no native frontmatter handling, so the leading
`---`-fenced block renders as visible body text. This preprocessor removes that
block at build time, uniformly across both writer paths, leaving body content
(including any mid-document `---` thematic break) byte-unchanged.

Protocol (mdBook): invoked as `<cmd> supports <renderer>` to advertise renderer
support (exit 0 = supported); otherwise reads `[context, book]` JSON from stdin
and writes the modified `book` object to stdout.

Standard library only (json, re, sys).
"""

import json
import re
import sys

# Leading frontmatter only: anchored at start, non-greedy, single match. A `---`
# thematic break lower in the page is never touched; content with no leading
# block is returned unchanged.
_FRONTMATTER = re.compile(r"^---\n.*?\n---\n", re.DOTALL)


def strip_frontmatter(content: str) -> str:
    return _FRONTMATTER.sub("", content, count=1)


def process_items(items: list) -> None:
    """Strip frontmatter from every Chapter, recursing into sub_items."""
    for item in items:
        chapter = item.get("Chapter") if isinstance(item, dict) else None
        if chapter is None:
            continue
        if isinstance(chapter.get("content"), str):
            chapter["content"] = strip_frontmatter(chapter["content"])
        sub_items = chapter.get("sub_items")
        if isinstance(sub_items, list):
            process_items(sub_items)


def main() -> int:
    # `supports <renderer>` → exit 0 (we support all renderers).
    if len(sys.argv) >= 2 and sys.argv[1] == "supports":
        return 0

    context, book = json.loads(sys.stdin.read())
    process_items(book.get("items", []))
    json.dump(book, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
