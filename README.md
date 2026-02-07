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
- 🧮 **Math Detection**: Automatic math expression detection with `<math>...</math>` markup
- 📑 **Caption Separation**: Automatic figure/table caption detection and separation

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
    let mut config = ParserConfig::new(); // LLM enabled by default
    let verbose = true;

    // Specify URL or local file path
    let url = "https://arxiv.org/pdf/1706.03762";

    // Parse PDF and get page structure
    let pages = parse(url, &mut config, verbose).await?;

    // Convert to section structure (basic)
    let sections = Section::from_pages(&pages);

    // Or with math markup (math expressions wrapped in <math>...</math> tags)
    let sections_with_math = Section::from_pages_with_math(&pages, &config.math_texts);

    // Output in JSON format
    let json = serde_json::to_string_pretty(&sections_with_math)?;
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

# Disable math markup (skip math detection)
rsrpp --pdf ./paper.pdf --out output.json --no-math-markup

# Include captions in main content instead of separate field
rsrpp --pdf ./paper.pdf --out output.json --include-captions

# Disable LLM-enhanced processing
rsrpp --pdf ./paper.pdf --out output.json --no-llm
```

##### CLI Options

| Option | Description |
|--------|-------------|
| `--pdf <URL\|PATH>` | Input PDF (URL or local file path) |
| `--out <PATH>` | Output JSON file path (default: output.json) |
| `--verbose` | Enable verbose output |
| `--no-llm` | Disable LLM-enhanced processing |
| `--include-captions` | Include captions in main content field |
| `--no-math-markup` | Disable math detection and markup |

##### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key (required for LLM features) | - |
| `OPENAI_API_MODEL` | Model to use for LLM processing | `gpt-5.2` |

## 📝 Output Format

The parser outputs JSON with the following structure:

```json
[
  {
    "index": 0,
    "title": "Abstract",
    "contents": ["This paper presents...", "Our approach achieves..."],
    "math_contents": ["This paper presents...", "Our approach achieves α = 0.95..."],
    "captions": []
  },
  {
    "index": 1,
    "title": "1 Introduction",
    "contents": ["Deep learning has..."],
    "math_contents": ["Deep learning has <math>f(x) = Wx + b</math>..."],
    "captions": ["Figure 1: Overview of our proposed method."]
  }
]
```

### Section Fields

| Field | Type | Description |
|-------|------|-------------|
| `index` | `i16` | Section order in document |
| `title` | `String` | Section title (e.g., "Abstract", "1 Introduction") |
| `contents` | `Vec<String>` | Original text content (captions excluded) |
| `math_contents` | `Option<Vec<String>>` | Text with math expressions wrapped in `<math>...</math>` tags (only present if math detected) |
| `captions` | `Vec<String>` | Figure/table captions belonging to this section (empty array if none) |

### Math Markup

Math expressions are detected using a multi-layered approach:

1. **LLM path** (when `OPENAI_API_KEY` is set): Extracts math via vision LLM, then aligns results to individual text blocks using trigram-based fuzzy matching
2. **Heuristic path** (fallback): Pattern-based detection with context analysis

Detected patterns include:
- Greek letters (α, β, γ, etc.)
- Mathematical operators (∑, ∏, ∫, ≤, ≥, ∈, etc.)
- Multi-character math functions (`sin(x)`, `cos(θ)`, `log(n)`, etc.)
- ASCII exponents and subscripts (`x^2`, `x_i`, `a_{n+1}`)
- Letter fractions (`a/b`)
- Norm notation (`||w||`)
- Common equation patterns with context-based validation

False positive filtering excludes:
- Date patterns (`2019/2020`)
- Statistical reporting (`n = 50 participants`)
- Section/figure references (`Section 3.1`, `Figure 2`)

All math output is unified to **LaTeX format** inside `<math>...</math>` tags:
```
Original: "The learning rate α converges when x^2 ≤ β"
Marked:   "The learning rate <math>\alpha</math> converges when <math>x^2 \leq \beta</math>"
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
  - `Block`: Block-level information and line collections (with `BlockType`: Body, Caption, Header)
  - `Page`: Page-level information and block collections
  - `Section`: Section structure with `contents`, `math_contents`, and `captions` fields
  - `RichText`: Text with original and math-marked versions
  - `fix_suffix_hyphens`: Text normalization for compound words (e.g., "databased" → "data-based")

- **`cleaner`**: Text cleaning and block classification
  - Figure/table caption detection (e.g., "Figure 1:", "Table 2.")
  - Block type classification (Body, Caption, Header)

- **`llm`**: LLM-enhanced processing and math detection
  - Math expression detection using Unicode patterns, structural heuristics, and context analysis
  - LLM-based math extraction with trigram alignment to text blocks
  - False positive filtering (dates, statistics, section references)
  - Unicode-to-LaTeX conversion for unified `<math>...</math>` output
  - LLM-based section validation

- **`extracter`**: Figure and table extraction functionality
  - Figure detection using OpenCV
  - Table region identification and exclusion (with area cap to reject false positives)
  - Text area degenerate detection with full-page fallback

- **`converter`**: Format conversion functionality
  - Page to section conversion
  - Section detection with fallback for non-standard formats (anchor-word matching)
  - JSON output generation

- **`config`**: Configuration management
  - Parser configuration management
  - Temporary file management
  - Math text mapping storage

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
<summary>1.0.25</summary>

- LLM-enhanced processing is now enabled by default (`ParserConfig::new()` sets `use_llm: true`)
  - If `OPENAI_API_KEY` is not set, LLM is automatically disabled at runtime
  - Use `--no-llm` (CLI) or `config.use_llm = false` (library) to explicitly disable
- Fixed LLM section validation discarding sections from pages the LLM hadn't examined
  - `merge_sections()` now uses page-range-aware logic: only validates sections within the LLM-examined page range
  - Sections outside the LLM page range are preserved from font-based detection

</details>

<details>
<summary>1.0.24</summary>

- Fixed body text loss in Nature-format and non-standard papers:
  - Added section detection fallback for papers without "Abstract" heading (e.g., Nature format) using anchor-word matching
  - Added text area degenerate detection to prevent filtering out all blocks when computed text area is too small
  - Capped table detection regions at 50% of page area to reject false positives from chart gridlines and figure borders
  - Exempted known section titles from table-region filtering
- Improved math extraction accuracy:
  - Fixed critical bug where LLM-extracted math text was discarded; added trigram-based block alignment
  - Reduced false positives: dates (`2019/2020`), statistics (`n = 50 participants`), section references
  - Added detection for multi-char math functions (`sin`, `cos`, `log`), ASCII exponents/subscripts (`x^2`, `x_i`), letter fractions (`a/b`), norm notation (`||w||`)
  - Unified math output to LaTeX format inside `<math>` tags (Unicode symbols converted to LaTeX commands)
  - Added context-based validation for structure-only pattern matches
  - Added 25 new tests including comprehensive regression suite

</details>

<details>
<summary>1.0.23</summary>

- Updated crate documentation and version.

</details>

<details>
<summary>1.0.22</summary>

- Added text cleaning and math markup support:
  - New `cleaner` module for caption detection (Figure, Table, Algorithm, etc.)
  - New `llm` module for math expression detection and `<math>...</math>` markup
  - `Section` now has `math_contents` and `captions` fields
  - New `Section::from_pages_with_math()` method for math-marked output
- New CLI options:
  - `--include-captions`: Include captions in main content field
  - `--no-math-markup`: Disable math detection and tagging
  - `--no-llm`: Disable LLM-enhanced processing
- New environment variable `OPENAI_API_MODEL` to specify LLM model (default: gpt-5.2)

</details>

<details>
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
