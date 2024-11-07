# Rust Research Paper Parser (rsrpp)

[![CircleCI](https://dl.circleci.com/status-badge/img/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main)

## Getting Started

### Installation

```sh
cargo install rsrpp
```

### Examples

Here are a few examples to help you get started with `rsrpp`:

#### Parsing a Research Paper

To parse a research paper, use the following command:

```rust
use rsrpp::parser::{ParseConfig, parse};

#[tokio::main]
async fn main() {
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/2410.24080";
    let res = parse(url, &mut config).await;
    let pages = res.unwrap(); // Vec<Page>
}
```

#### Extract Texts as json

```rust
use rsrpp::parser::{ParseConfig, parse};

#[tokio::main]
async fn main() {
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/1706.03762";
    let pages = parse(url, &mut config).await.unwrap();
    let json = pages2json(&pages);
}
```

For more detailed usage and options, refer to the [source code](https://github.com/akitenkrad/rsrpp).
