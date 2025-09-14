# Rust Research Paper Parser (rsrpp)

[![CircleCI](https://dl.circleci.com/status-badge/img/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/circleci/X1fiE4koKU88Z9sKwWoPAH/S2NQ8VZz6F1CZ6vuvFBE3Y/tree/main)
![Crates.io Version](https://img.shields.io/crates/v/rsrpp?style=flat-square)

## RuSt Research Paper Parser (rsrpp)

The `rsrpp` library provides a set of tools for parsing research papers.

<img src="../RSRPP.png" alt="LOGO" width="150" height="150"/>

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

<details open>
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

- Update: The max retry time for saving PDF files has been increased.

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
