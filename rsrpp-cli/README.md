# rsrpp — Rust Research Paper Parser (CLI)

![Crates.io Version](https://img.shields.io/crates/v/rsrpp-cli?style=flat-square)
![License: MIT](https://img.shields.io/crates/l/rsrpp-cli?style=flat-square)

Parses a research paper PDF into structured JSON sections: body text, figure and table
captions, math, and optionally references. Command-line front end for the
[`rsrpp`](https://crates.io/crates/rsrpp) library.

## Install

```bash
sudo apt install poppler-utils libopencv-dev clang libclang-dev   # Ubuntu/Debian
brew install poppler opencv pkg-config                            # macOS

cargo install rsrpp-cli
```

OpenCV is linked dynamically and an upgrade can break an installed binary. Building with
`cargo install rsrpp-cli --no-default-features` produces a binary with no OpenCV linkage
at all — see
[Installation](https://github.com/akitenkrad/rsrpp/blob/main/docs/install.md#opencv-linking).

## Usage

```bash
rsrpp --pdf "https://arxiv.org/pdf/1706.03762" --out attention_paper.json --verbose
rsrpp --pdf ./paper.pdf --out output.json --no-llm
```

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

## Exit codes

A run over a corpus has to tell *skip this paper* from *stop, something here is wrong*:

| Code | Meaning |
|------|---------|
| `0` | The JSON was written |
| `1` | The input or the environment failed |
| `2` | The command line was wrong |
| `3` | Poppler was stopped at one of its limits — this document is not going to finish |

Nothing exits by panicking, and no backtrace is printed. Failures put one line on stderr.

## Documentation

- [CLI reference](https://github.com/akitenkrad/rsrpp/blob/main/docs/cli.md)
- [Output format](https://github.com/akitenkrad/rsrpp/blob/main/docs/output.md)
- [Release history](https://github.com/akitenkrad/rsrpp/blob/main/docs/releases.md)
- [日本語のドキュメント](https://github.com/akitenkrad/rsrpp/blob/main/README.ja.md)

## License

MIT.
