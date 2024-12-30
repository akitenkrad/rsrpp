# Rust Research Paper Parser (rsrpp)

[![CircleCI](https://dl.circleci.com/status-badge/img/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main)
![Crates.io Version](https://img.shields.io/crates/v/rsrpp?style=flat-square)

## RuSt Research Paper Parser (rsrpp)

The `rsrpp` library provides a set of tools for parsing research papers.

<img src="LOGO.png" alt="LOGO" width="150" height="150"/>

### Quick Start

#### Pre-requirements

- Poppler: `sudo apt install poppler-utils`
- OpenCV: `sudo apt install libopencv-dev clang libclang-dev`

#### Installation

To start using the `rsrpp` library, add it to your project's dependencies in the `Cargo.toml` file:

```bash
cargo add rsrpp
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
let sections = Section::from_pages(&pages); // Vec<Section>
let json = serde_json::to_string(&sections).unwrap(); // String
```

### Tests

The library includes a set of tests to ensure its functionality. To run the tests, use the following command:

```sh
cargo test
```

License: MIT

## Releases

<details open>
<summary>1.0.9</summary>

- Updated: implemented new errors to handle invalid URLs.

</details>

<details>
<summary>1.0.8</summary>

- Updated: The max retry time for saving PDF files has been increased.

</details>

<details>
<summary>1.0.7</summary>

- Fix bugs: After converting to PDF, the program now waits until processing is complete.

</details>

<details>
<summary>1.0.4</summary>

- Fixed bugs in `get_pdf_info`.
- Made minor improvements.

</details>

<details>
<summary>1.0.3</summary>

- Added cli -> [rsrpp-cli](https://crates.io/crates/rsrpp-cli).

</details>

<details>
<summary>1.0.2</summary>

- Updated the `Section` module. `content: String` was replaced by `content: Vec<TextBlock>`.

</details>
