//! # RuSt Research Paper Parser (rsrpp)
//!
//! The `rsrpp` library provides a set of tools for parsing research papers.
//!
//! ## Quick Start
//!
//! ### Pre-requirements
//! - Poppler: `sudo apt install poppler-utils`
//! - OpenCV: `sudo apt install libopencv-dev clang libclang-dev`
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
//! Here is a simple example of how to use the parser module:
//!
//! ```rust
//! # use rsrpp::parser::structs::{ParserConfig, Section};
//! # use rsrpp::parser::{parse, pages2json};
//! # async fn try_main() -> Result<(), String> {
//! let mut config = ParserConfig::new();
//! let verbose = true;
//! let url = "https://arxiv.org/pdf/1706.03762";
//! let pages = parse(url, &mut config, verbose).await.unwrap(); // Vec<Page>
//! let sections = Section::from_pages(&pages); // Vec<Section>
//! let json = serde_json::to_string(&sections).unwrap(); // String
//! # Ok(())
//! # }
//! # #[tokio::main]
//! # async fn main() {
//! #    try_main().await.unwrap();
//! # }
//! ```
//!
//! ## Tests
//!
//! The library includes a set of tests to ensure its functionality. To run the tests, use the following command:
//!
//! ```sh
//! cargo test
//! ```

pub mod config;
pub mod converter;
pub mod extracter;
pub mod models;
pub mod parser;
pub mod test_utils;
