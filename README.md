<p align="center"><img src="docs/assets/hero.svg" width="100%"></p>

**English** | [日本語](README.ja.md)

![Crates.io Version](https://img.shields.io/crates/v/rsrpp?style=flat-square)
![License: MIT](https://img.shields.io/crates/l/rsrpp?style=flat-square)
![GitHub repo size](https://img.shields.io/github/repo-size/akitenkrad/rsrpp?style=flat-square)

# Rust Research Paper Parser (RSRPP)

<img src="LOGO.png" alt="RSRPP Logo" width="150" height="150" align="right"/>

RSRPP turns a research paper PDF into structured sections. It drives poppler for the
text and its geometry, works out the column layout and which lines are section
headings, and emits JSON in which every section carries its body text, its figure and
table captions, its math, and — optionally — its references. It ships as a Rust library
and as a CLI, and it is built to run over a corpus rather than one paper at a time.

## Install

```bash
# Ubuntu/Debian
sudo apt install poppler-utils libopencv-dev clang libclang-dev

# macOS (Homebrew)
brew install poppler opencv pkg-config

# Fedora/RHEL
sudo dnf install poppler-utils opencv-devel clang clang-devel
```

```bash
cargo add rsrpp          # library
cargo install rsrpp-cli  # CLI
```

OpenCV is linked dynamically and only the default `table-detection` feature needs it;
see [Installation](docs/install.md) before installing the CLI.

```bash
rsrpp --pdf "https://arxiv.org/pdf/1706.03762" --out attention_paper.json --verbose
```

## Documentation

- [What it is for](docs/usecases.md) — the problems this parser is shaped around
- [Installation](docs/install.md) — prerequisites, OpenCV linking, the `table-detection` feature
- [CLI](docs/cli.md) — options, environment variables, exit codes
- [Library](docs/library.md) — `ParserConfig`, limits, temporary files, error handling
- [Output format](docs/output.md) — section fields, math markup, text coverage
- [Architecture](docs/architecture.md) — the modules and what each decides
- [Testing](docs/testing.md) — unit vs end-to-end, and measuring a change over a corpus
- [Releases](docs/releases.md) — version history

## License

MIT. See [LICENSE](LICENSE).

- [Crates.io — rsrpp](https://crates.io/crates/rsrpp)
- [Crates.io — rsrpp-cli](https://crates.io/crates/rsrpp-cli)
