**English** | [日本語](cli.ja.md)

# CLI

```bash
# Parse arXiv paper
rsrpp --pdf "https://arxiv.org/pdf/1706.03762" --out attention_paper.json --verbose

# Parse local file
rsrpp --pdf ./paper.pdf --out output.json

# Disable math markup (skip math detection)
rsrpp --pdf ./paper.pdf --out output.json --no-math-markup

# Include captions in main content instead of separate field
rsrpp --pdf ./paper.pdf --out output.json --include-captions

# Disable LLM-enhanced processing
rsrpp --pdf ./paper.pdf --out output.json --no-llm
```

## Options

| Option | Description |
|--------|-------------|
| `--pdf <URL\|PATH>` | Input PDF (URL or local file path) |
| `--out <PATH>` | Output JSON file path (default: output.json) |
| `--verbose` | Enable verbose output |
| `--no-llm` | Disable LLM-enhanced processing |
| `--include-captions` | Include captions in main content field |
| `--no-math-markup` | Disable math detection and markup |
| `--extract-references` | Extract structured references (requires `OPENAI_API_KEY`) |
| `--keep-dropped` | Append an `Unassigned` section holding everything the filters discarded |

## Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key (required for LLM features) | - |
| `OPENAI_API_MODEL` | Model to use for LLM processing | `gpt-5.2` |

## Exit codes

A run over a corpus has to tell *skip this paper and carry on* from *stop, something
here is wrong*. The exit code says which:

| Code | Meaning |
|------|---------|
| `0` | The JSON was written |
| `1` | The input or the environment failed: the PDF is missing or unreadable, the output path cannot be written, the output name is not `.json`, poppler is not installed |
| `2` | The command line was wrong (an unknown flag, a missing `--pdf`) |
| `3` | Poppler was stopped at one of its limits — this document is not going to finish, and the batch should move on |

Nothing exits by panicking, and no backtrace is printed. Failures put a single line on
stderr:

```
rsrpp: pdftohtml exceeded the output limit of 3400 files and was killed (poppler_max_files_per_page); ...
```

Code `3` is the one a batch script should treat as "give up on this paper". It comes
from [the poppler limits](library.md#limits-on-poppler), and the message names which
limit was hit. Everything else deserves attention: a `1` usually means the environment
is not what the script assumed.

## Text coverage

Some text is removed on purpose: cells inside a detected table region, and blocks too
narrow and short to be body text (figure axis labels, legends). To keep "removed on
purpose" from turning into "lost without saying so", every parse reports what it kept:

```
WARN rsrpp::parser: Text coverage 93.2% (32018 of 34364 source chars kept, 32018 chars
in blocks). Discarded — narrow block: 196 fragments / 1128 chars, outside text area: 2
fragments / 62 chars, table region: 145 fragments / 1156 chars.
```

The line is logged at `WARN` below 99% coverage and at `INFO` above it, so a badly
mis-parsed paper is visible without diffing the output. Coverage counts alphanumeric
characters only, so whitespace and reading-order differences do not register as loss.

`--keep-dropped` appends an `Unassigned` section carrying the discarded fragments,
which makes the output lossless with respect to what poppler read:

```bash
rsrpp --pdf paper.pdf --out output.json --keep-dropped --include-captions
```

Reach for it whenever a paper looks like it lost text: most of the time the text is
still there, in the drop ledger rather than in a section.
