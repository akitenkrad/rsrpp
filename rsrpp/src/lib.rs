//! # RuSt Research Paper Parser (rsrpp)
//!
//! The `rsrpp` library provides a set of tools for parsing research papers.
//!
//! ## Features
//!
//! - Extract structured text from PDF papers (sections, paragraphs)
//! - Robust section detection with fallback for non-standard formats (Nature, etc.)
//! - Detect and separate figure/table captions
//! - Math expression detection and LaTeX-formatted markup (heuristic + LLM with trigram alignment)
//! - **Structured reference extraction** (LLM-based, requires `OPENAI_API_KEY`)
//!
//! ## Quick Start
//!
//! ### Pre-requirements
//! - Poppler: `sudo apt install poppler-utils`
//! - OpenCV 4.x: `sudo apt install libopencv-dev clang libclang-dev` (only needed for the
//!   default `table-detection` feature; build with `--no-default-features` to drop it)
//! - `OPENAI_API_KEY` environment variable for LLM features (enabled by default; auto-disabled if not set)
//!
//! ### Cargo features
//!
//! - `table-detection` *(default)* — OpenCV-backed detection of table regions, so table
//!   contents are excluded from the body text. Disabling it removes the OpenCV dependency
//!   entirely, at the cost of table text being merged into the surrounding section.
//!
//! ### Installation
//! To start using the `rsrpp` library, add it to your project's dependencies in the `Cargo.toml` file:
//!
//! ```bash
//! cargo add rsrpp
//! ```
//!
//! Then, import the necessary modules in your code:
//!
//! ```rust
//! extern crate rsrpp;
//! use rsrpp::parser;
//! ```
//!
//! ## Examples
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! # use rsrpp::config::ParserConfig;
//! # use rsrpp::models::Section;
//! # use rsrpp::parser::parse;
//! # async fn try_main() -> Result<(), String> {
//! let mut config = ParserConfig::new(); // LLM enabled by default
//! let verbose = true;
//! let url = "https://arxiv.org/pdf/1706.03762";
//! let pages = parse(url, &mut config, verbose).await.unwrap(); // Vec<Page>
//!
//! // Basic conversion (captions separated, no math markup)
//! let sections = Section::from_pages(&pages); // Vec<Section>
//!
//! // With math markup (math expressions wrapped in <math>...</math> tags, LaTeX format)
//! let sections_with_math = Section::from_pages_with_math(&pages, &config.math_texts);
//!
//! let json = serde_json::to_string(&sections_with_math).unwrap(); // String
//! # Ok(())
//! # }
//! # #[tokio::main]
//! # async fn main() {
//! #    try_main().await.unwrap();
//! # }
//! ```
//!
//! ### With Reference Extraction (requires OPENAI_API_KEY)
//!
//! ```rust,ignore
//! use rsrpp::config::ParserConfig;
//! use rsrpp::parser::{parse, pages2paper_output};
//!
//! let mut config = ParserConfig::new(); // LLM enabled by default
//! config.extract_references = true; // Enable reference extraction
//!
//! let pages = parse("paper.pdf", &mut config, false).await?;
//! let output = pages2paper_output(&pages, &config); // PaperOutput
//!
//! // output.sections - Vec<Section>
//! // output.references - Vec<Reference> with authors, title, year, venue, etc.
//! ```
//!
//! ## Temporary files
//!
//! A parse works in a directory of its own under the system temp directory, and
//! everything it produced is removed when the last handle to the [`config::ParserConfig`]
//! goes away — on the normal path, on an early return, and while a panic unwinds.
//! [`config::ParserConfig::clean_files`] removes the same directory; calling it is
//! optional, and calling it twice is not an error.
//!
//! Two consequences are worth knowing. A path copied out of `config.pdf_figures` does
//! not outlive the config: read the file while the config is alive. And a process
//! killed with `SIGKILL` runs no destructor, so it leaves its directory behind; the
//! next parse reclaims what earlier runs abandoned, taking only directories whose
//! owning process is provably gone, so parses running side by side never take each
//! other's files. [`tempdir_sweep::sweep_abandoned_temp_dirs`] triggers that directly.
//!
//! ## Limits on poppler
//!
//! Some PDFs make poppler write without bound — a figure painted with a one-point
//! tiling pattern has `pdftohtml` emit a PNG per tile, which is millions of files and
//! gigabytes from a single parse. Every poppler invocation is watched and killed if it
//! passes either a time limit or an output limit; both are needed, because a very long
//! scanned document is slow for an honest reason and has to be told apart from one that
//! will never finish. The budgets are per page of the document, with a floor for short
//! ones:
//!
//! - [`config::DEFAULT_POPPLER_TIMEOUT`] (900s)
//! - [`config::DEFAULT_POPPLER_MAX_FILES_PER_PAGE`] (8,000)
//! - [`config::DEFAULT_POPPLER_MAX_BYTES_PER_PAGE`] (20 MiB)
//!
//! A breach is reported as [`converter::PopplerLimitError`], so a caller can recognise
//! it by type rather than by matching on the message:
//!
//! ```rust,ignore
//! use rsrpp::converter::{PopplerLimit, PopplerLimitError};
//!
//! match parse(url, &mut config, false).await {
//!     Ok(pages) => { /* ... */ }
//!     Err(e) => match e.downcast_ref::<PopplerLimitError>().map(|e| e.limit()) {
//!         Some(PopplerLimit::Time) => eprintln!("too slow to finish"),
//!         Some(_) => eprintln!("writes without bound"),
//!         None => eprintln!("parsing error: {}", e),
//!     },
//! }
//! ```
//!
//! ## Tests
//!
//! The library includes a set of tests to ensure its functionality. To run the tests, use the following command:
//!
//! ```sh
//! cargo test
//! ```

pub mod cleaner;
pub mod config;
pub mod converter;
pub mod extracter;
#[allow(deprecated)]
pub mod llm;
pub mod models;
pub mod parser;
pub mod tempdir_sweep;
pub mod test_utils;
