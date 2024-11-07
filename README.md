# Rust Research Paper Parser (rsrpp)

[![CircleCI](https://dl.circleci.com/status-badge/img/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main)

The `rsrpp` library provides a set of tools for parsing research papers.

## Quick Start

### Pre-requisites

- Poppler: `sudo apt install poppler-utils`
- OpenCV: `sudo apt install libopencv-dev clang libclang-dev`

### Installation

To start using the `rsrpp` library, add it to your `Cargo.toml`:

```toml
[dependencies]
rsrpp = "1.0.0"
```

Then, import the necessary modules in your code:

```rust
extern crate rsrpp;
use rsrpp::parser;
```

### Examples

Here is a simple example of how to use the parser module:

```rust
let mut config = ParserConfig::new();
let url = "https://arxiv.org/pdf/1706.03762";
let pages = parse(url, &mut config).await.unwrap(); // Vec<Page>
let json = pages2json(&pages);
```

### Tests

The library includes a set of tests to ensure its functionality. To run the tests, use the following command:

```sh
cargo test
```

License: MIT
