**English** | [日本語](testing.ja.md)

# Testing

Unit tests and end-to-end tests are separate, because they cost three orders of
magnitude apart: a parse with the LLM enabled takes ~176s, the same parse without it
~2.9s, and the unit tests together ~2s.

```bash
# Unit tests only (~2s) — the one to run while working
makers nextest

# End-to-end: fetches PDFs, runs poppler and OpenCV, calls the OpenAI API
makers nextest-e2e

# Everything, including doc-tests — run before committing
makers nextest-all

# A single test
makers nextest test_parse_extract_sections_1

# Without cargo-make
cargo nextest run --workspace
```

The e2e tests download sample papers from arXiv and cache them (TTL via
`RSRPP_TEST_CACHE_TTL_SECONDS`, default 24h), and a few call the OpenAI API, so
`OPENAI_API_KEY` must be set for those to do anything. Tests whose assertions are
structural parse with the LLM disabled; the LLM path has its own dedicated tests.

## Measuring a change over a corpus

Passing tests are not the completion condition for a change to the parsing rules. The
tests cover the cases someone thought of; a corpus of real papers covers the ones
nobody did. Two of the worst regressions this parser has had — a stamp along the page
margin becoming the first section, and a document whose column layout changes partway
through — were invisible to the tests and to a 300-paper sample alike, and only showed
up over the full corpus.

[`tools/ab-harness/`](../tools/ab-harness/README.md) runs two builds over the same set
of PDFs and reports what moved: section lists that changed, coverage that dropped,
papers whose text got shorter. Bring your own corpus; nothing in this repository ships
paper files or paths to them.
