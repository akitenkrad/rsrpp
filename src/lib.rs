//! # RuSt Research Paper Parser (rsrpp)
//!
//! The `rsrpp` library provides a set of tools for parsing research papers.
//!
//! ## Quick Start
//!
//! To start using the `rsrpp` library, add it to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! rsrpp = "1.0.0"
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
//! # use rsrpp::parser::structs::ParserConfig;
//! # use rsrpp::parser::{parse, pages2json};
//! # async fn try_main() -> Result<(), String> {
//! let mut config = ParserConfig::new();
//! let url = "https://arxiv.org/pdf/1706.03762";
//! let pages = parse(url, &mut config).await.unwrap(); // Vec<Page>
//! let json = pages2json(&pages);
//! println!("Parsed text: {}", json);
//! # Ok(())
//! # }
//! # #[tokio::main]
//! # async fn main() {
//! #    try_main().await.unwrap();
//! # }
//!
//! ```
//!
//! ## Tests
//!
//! The library includes a set of tests to ensure its functionality. To run the tests, use the following command:
//!
//! ```sh
//! cargo test
//! ```
#[cfg(test)]
mod tests;

pub mod parser;
