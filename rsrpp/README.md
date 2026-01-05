# Rust Research Paper Parser (rsrpp)

[![CircleCI](https://dl.circleci.com/status-badge/img/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main)
![Crates.io Version](https://img.shields.io/crates/v/rsrpp?style=flat-square)

## RuSt Research Paper Parser (rsrpp)

The `rsrpp` library provides a set of tools for parsing research papers.

<img src="../LOGO.png" alt="LOGO" width="150" height="150"/>

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
<summary>1.0.21</summary>

- Fixed panic-causing unwrap() calls with proper error handling.

</details>

<details>
<summary>1.0.20</summary>

- Fixed Poppler 25.12.0 compatibility on macOS.

</details>

<details>
<summary>1.0.19</summary>

- Refactored `fix_suffix_hyphens` to support 31 compound word suffixes:
  - `-based`, `-driven`, `-oriented`, `-aware`, `-agnostic`, `-independent`, `-dependent`, `-first`, `-native`, `-centric`, `-intensive`, `-bound`, `-safe`, `-free`, `-proof`, `-efficient`, `-optimized`, `-enabled`, `-powered`, `-ready`, `-capable`, `-compatible`, `-compliant`, `-level`, `-scale`, `-wide`, `-specific`, `-friendly`, `-facing`, `-like`, `-style`
- Added unit tests for suffix hyphenation functionality.

</details>

<details>
<summary>1.0.18</summary>

- updated how to extract section titles from PDF.

</details>

<details>
<summary>1.0.17</summary>

- restructured `rsrpp.parser`.
- updated how to extract section titles from PDF.
- updated tests.

</details>

<details>
<summary>1.0.16</summary>

- removed `init_logger` form `rsrpp`.

</details>

<details>
<summary>1.0.15</summary>

- fixed typo.
- introdeced `tracing` logger.

</details>

<details>
<summary>1.0.14</summary>

- Updated `rsrpp` version for `rsrpp-cli`.

</details>

<details>
<summary>1.0.13</summary>

- Updated dependencies.
- removed build.sh because it requires sudo when installing the crate.

</details>

<details>
<summary>1.0.12</summary>

- Fixed a bug: remove unused `println!`.

</details>

<details>
<summary>1.0.11</summary>

- Fixed a bug in xml loop to finish when the file reaches to end.

</details>

<details>
<summary>1.0.10</summary>

- Added verbose mode.
- Fixed a bug in the process extracting page number.

</details>

<details>
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
