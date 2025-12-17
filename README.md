# Rust Research Paper Parser (RSRPP)

![Crates.io Version](https://img.shields.io/crates/v/rsrpp?style=flat-square)
![License: MIT](https://img.shields.io/crates/l/rsrpp?style=flat-square)
![GitHub repo size](https://img.shields.io/github/repo-size/akitenkrad/rsrpp?style=flat-square)

<img src="LOGO.png" alt="RSRPP Logo" width="150" height="150" align="right"/>

A high-performance Rust-based research paper PDF parser library. Extracts structured data from academic paper PDFs, including text, figures, tables, and section information in JSON format.

## ✨ Features

- 🚀 **High Performance**: Leverages Rust's safety and performance benefits
- 📄 **Comprehensive Analysis**: Automatically extracts text, figures, and section structures
- 🌐 **Flexible Input**: Supports both local files and URLs (arXiv, etc.)
- 🔧 **CLI Support**: Includes command-line tool `rsrpp-cli`
- 📊 **Structured Output**: Detailed document structure data in JSON format
- 🎯 **High Accuracy**: Figure detection and layout analysis using OpenCV

## 🚀 Quick Start

### Prerequisites

Before using RSRPP, install the following dependencies:

```bash
# Ubuntu/Debian
sudo apt install poppler-utils libopencv-dev clang libclang-dev

# macOS (Homebrew)
brew install poppler opencv pkg-config

# Fedora/RHEL
sudo dnf install poppler-utils opencv-devel clang clang-devel
```

### Installation

#### As a Library

```bash
cargo add rsrpp
```

#### As a CLI Tool

```bash
cargo install rsrpp-cli
```

### Basic Usage

#### As a Library

```rust
use rsrpp::parser::{parse, structs::{ParserConfig, Section}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ParserConfig::new();
    let verbose = true;
    
    // Specify URL or local file path
    let url = "https://arxiv.org/pdf/1706.03762";
    
    // Parse PDF and get page structure
    let pages = parse(url, &mut config, verbose).await?;
    
    // Convert to section structure
    let sections = Section::from_pages(&pages);
    
    // Output in JSON format
    let json = serde_json::to_string_pretty(&sections)?;
    println!("{}", json);
    
    // Clean up temporary files
    config.clean_files()?;
    
    Ok(())
}
```

#### As a CLI Tool

```bash
# Parse arXiv paper
rsrpp --pdf "https://arxiv.org/pdf/1706.03762" --out attention_paper.json --verbose

# Parse local file
rsrpp --pdf ./paper.pdf --out output.json
```

## 📚 Architecture

RSRPP consists of the following modules:

### Core Modules

- **`parser`**: Main PDF parsing logic
  - PDF to HTML conversion (using Poppler)
  - HTML structure parsing and page object generation
  - Section structure extraction

- **`models`**: Data structure definitions
  - `Word`: Word-level information (coordinates, font size, etc.)
  - `Line`: Line-level information and word collections
  - `Block`: Block-level information and line collections  
  - `Page`: Page-level information and block collections
  - `Section`: Section structure (Abstract, Introduction, etc.)

- **`extracter`**: Figure and table extraction functionality
  - Figure detection using OpenCV
  - Table region identification and exclusion

- **`converter`**: Format conversion functionality
  - Page to section conversion
  - JSON output generation

- **`config`**: Configuration management
  - Parser configuration management
  - Temporary file management

### CLI Tool

`rsrpp-cli` is provided as an independent binary with the following features:

- Command-line argument parsing
- Logging functionality
- JSON output file saving

## 🔧 Advanced Usage

### Custom Configuration

```rust
use rsrpp::parser::structs::ParserConfig;

let mut config = ParserConfig::new();
// Customize configuration
// config.some_setting = value;

let pages = parse("path/to/paper.pdf", &mut config, true).await?;
```

### Error Handling

```rust
use rsrpp::parser::parse;
use anyhow::Result;

async fn parse_paper(url: &str) -> Result<()> {
    let mut config = ParserConfig::new();
    
    match parse(url, &mut config, true).await {
        Ok(pages) => {
            println!("Parsing completed: {} pages", pages.len());
            // Continue processing...
        }
        Err(e) => {
            eprintln!("Parsing error: {}", e);
            return Err(e);
        }
    }
    
    Ok(())
}
```

## 🧪 テスト

## 🧪 Testing

The project includes a comprehensive test suite:

```bash
# Run all tests
makers nextest
```

### Development Environment Setup

```bash
git clone https://github.com/akitenkrad/rsrpp.git
cd rsrpp
makers build-all
cargo nextest
```

## 📄 License

This project is released under the MIT License. See the [LICENSE](LICENSE) file for details.

Note: This project is based on rsrpp by Aki.

## 🔗 Related Links

- [Crates.io - rsrpp](https://crates.io/crates/rsrpp)
- [Crates.io - rsrpp-cli](https://crates.io/crates/rsrpp-cli)
- [GitHub Repository](https://github.com/akitenkrad/rsrpp)

---

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
