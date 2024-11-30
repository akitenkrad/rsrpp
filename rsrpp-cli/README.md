# Rust Research Paper Parser (rsrpp)

[![CircleCI](https://dl.circleci.com/status-badge/img/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main)

## RuSt Research Paper Parser (rsrpp)

The `rsrpp` library provides a set of tools for parsing research papers.

### Quick Start

#### Pre-requirements

- Poppler: `sudo apt install poppler-utils`
- OpenCV: `sudo apt install libopencv-dev clang libclang-dev`

#### Installation

To start using the `rsrpp` library, add it to your project's dependencies in the `Cargo.toml` file:

```bash
cargo install rsrpp-cli
rsrpp --help
A Rust project for research paper pdf.

Usage: rsrpp [OPTIONS] --pdf <PDF>

Options:
  -p, --pdf <PDF>  
  -o, --out <OUT>  
  -h, --help       Print help
  -V, --version    Print version
```

## Releases

### 1.0.2

- Updated the `Section` module. `content: String` was replaced by `content: Vec<TextBlock>`.

### 1.0.3

- added cli -> [rsrpp-cli](https://crates.io/crates/rsrpp-cli).
