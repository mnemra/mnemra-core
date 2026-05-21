---
description: "Run the docs translation pipeline: compute stale pages, dispatch translation agents in parallel, update the manifest."
---

# /docs-translate

Runs the mnemra-core docs translation pipeline. Produces `docs/_published/human/` (EXPLAIN pass) and `docs/_published/agent/` (STRIP pass) from `docs/src/`.

## Step 1 — Compute the plan

Run via Bash, capture stdout:

```bash
uv run scripts/docs-translate.py --plan --src docs/src --out docs/_published --prompts docs/prompts
```

Parse the JSON from stdout. If `items` is empty, report "no translation needed" and stop.

## Step 2 — Dispatch translation agents (parallelism is mandatory)

For each item in `items`, dispatch an Agent with `subagent_type: general-purpose`, `mode: default`, using the item's `assembled_prompt` as the task input. The agent's job is to return the translated Markdown as its final message — no preamble, no commentary, no explanation. The dispatched Agent has no Write, Edit, or Bash access. Its only job is to return the translated Markdown as its final message. The parent session does the file write.

**Parallelism is mandatory and explicit: dispatch agents in batches of 4 by sending exactly 4 Agent tool calls in a single message.** Claude only parallelizes Agent calls when they appear together in a single tool-use message. Serial dispatch (one Agent call per message) is a defect — it processes pages sequentially and is dramatically slower.

For a tree of 7 pages: send 4 Agent calls in one message (batch 1), then 3 Agent calls in the next message (batch 2).

After each batch returns:

- For each Agent response: if the response is empty or begins with explanation prose rather than Markdown content, retry once with the prompt prefixed by: `Return only the translated Markdown document. Do not include any preamble, commentary, or explanation. Begin immediately with the document content.`
- If the retry also fails: report the failing page path and abort. Do not proceed to `--finalize` with missing outputs.
- Write the translated Markdown to `output_path` using the Write tool.

## Step 3 — Finalize

After all items are written to disk, run:

```bash
uv run scripts/docs-translate.py --finalize --src docs/src --out docs/_published --prompts docs/prompts
```

If `--finalize` exits non-zero: report which outputs are missing and stop. Do not attempt a partial manifest update.

## Step 4 — Report

On success: report a one-line summary: `N pages translated, manifest updated.`

Include the paths of any pages that were skipped (already up-to-date per the hash gate) so Peter knows what was touched.
