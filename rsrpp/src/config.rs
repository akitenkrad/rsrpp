use anyhow::Result;
use rand::RngExt;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::models::Reference;
use serde::{Deserialize, Serialize};

pub type PageNumber = i16;

/// Counts the alphanumeric characters in `text`.
///
/// Coverage is compared on alphanumerics only so that whitespace and layout
/// differences between poppler's reading order and the block structure do not
/// register as lost text.
pub(crate) fn normalized_len(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// Why a piece of text was discarded during parsing.
///
/// Every variant corresponds to one geometric or heuristic filter in the pipeline.
/// Discarded text is recorded in [`ParserConfig::dropped_texts`] rather than thrown
/// away silently, so callers can audit what a parse left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DropReason {
    /// The line sits inside a region detected as a table.
    TableRegion,
    /// The block does not overlap the page's dominant text area at all.
    OutsideTextArea,
    /// The block is too narrow and too short to be body text.
    NarrowBlock,
}

impl std::fmt::Display for DropReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DropReason::TableRegion => "table region",
            DropReason::OutsideTextArea => "outside text area",
            DropReason::NarrowBlock => "narrow block",
        };
        f.write_str(s)
    }
}

/// A fragment of text that a filter removed from the parsed output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedText {
    pub page: PageNumber,
    pub reason: DropReason,
    pub text: String,
}

/// `ParserConfig` is a configuration structure for parsing PDF documents.
///
/// # Fields
///
/// * `pdf_path` - The file path to the PDF document.
/// * `pdf_text_path` - The file path to the extracted text from the PDF document.
/// * `pdf_figures` - A map of page numbers to file paths of extracted figures from the PDF document.
/// * `pdf_xml_path` - The file path to the extracted XML data from the PDF document.
/// * `sections` - A vector of tuples containing page numbers and section titles.
/// * `pdf_info` - A map containing metadata information about the PDF document.
/// * `use_llm` - Whether to use LLM for enhanced math extraction.
/// * `math_texts` - A map of (page_number, block_index) to math-marked text.
/// * `extract_references` - Whether to extract structured references from the paper.
/// * `references` - Extracted bibliographic references.
/// * `dropped_texts` - Text removed by the parsing filters, with the reason for each drop.
/// * `source_char_count` - Normalized character count of the source text, the denominator of `coverage`.
///
/// # Methods
///
/// * `new` - Creates a new instance of `ParserConfig` with default values.
/// * `pdf_width` - Returns the width of the PDF document as an `i32`.
/// * `pdf_height` - Returns the height of the PDF document as an `i32`.
/// * `clean_files` - Removes the PDF, text, XML, and figure files associated with the `ParserConfig`.
/// * `reset_document_state` - Clears everything derived from a previously parsed document.
/// * `coverage` - Fraction of the source text that survived the parsing filters.
/// * `dropped_char_count` - Normalized character count of everything that was discarded.
//
/// Construct with [`ParserConfig::new`] and adjust the settings you need.
///
/// Marked `#[non_exhaustive]` because this type doubles as settings and as the place
/// results are reported: the result fields grow whenever the parser learns to report
/// something new, and each addition would otherwise break every downstream struct
/// literal. `new()` has always been the only sensible way to build one.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ParserConfig {
    pub pdf_path: String,
    pub pdf_text_path: String,
    pub pdf_figures: HashMap<PageNumber, String>,
    pub pdf_xml_path: String,
    pub sections: Vec<(PageNumber, String)>,
    pub pdf_info: HashMap<String, String>,
    pub use_llm: bool,
    pub math_texts: HashMap<(PageNumber, usize), String>,
    pub extract_references: bool,
    pub references: Vec<Reference>,
    /// Text removed by the geometric/heuristic filters, with the reason for each drop.
    /// Populated during parsing; never dropped silently.
    pub dropped_texts: Vec<DroppedText>,
    /// Number of normalized characters poppler produced for this PDF, i.e. the
    /// denominator of [`ParserConfig::coverage`]. `0` until parsing has run.
    pub source_char_count: usize,
}

impl ParserConfig {
    /// Creates a new `ParserConfig` instance with default values.
    ///
    /// This function initializes the following fields:
    /// - `pdf_path`: A randomly generated file path in the `/tmp` directory.
    /// - `pdf_text_path`: The path to the HTML text version of the PDF.
    /// - `pdf_figures`: An empty `HashMap` to store figures extracted from the PDF.
    /// - `pdf_xml_path`: The path to the raw XML version of the PDF.
    /// - `sections`: An empty vector to store sections of the parsed PDF.
    /// - `pdf_info`: An empty `HashMap` to store additional PDF information.
    ///
    /// # Returns
    ///
    /// A new `ParserConfig` instance with the initialized fields.
    pub fn new() -> ParserConfig {
        // Unique by construction, not by luck. Two configs alive at once must never
        // share a path: they delete their own files in `clean_files`, so a collision
        // makes one parse quietly remove the other's PDF. A random number alone could
        // repeat, which is why the process id and a counter come first; the random
        // part only keeps paths unpredictable across runs.
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let mut rng = rand::rng();
        let pdf_path = format!(
            "{}/pdf_{}_{}_{}.pdf",
            std::env::temp_dir().display(),
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed),
            rng.random_range(10000..99999)
        );

        let pdf_figures = HashMap::new();
        let pdf_html_path = pdf_path.clone().replace(".pdf", ".text.html");
        let pdf_raw_html_path = pdf_path.clone().replace(".pdf", ".xml");
        let sections = Vec::new();
        ParserConfig {
            pdf_path: pdf_path,
            pdf_text_path: pdf_html_path,
            pdf_figures: pdf_figures,
            pdf_xml_path: pdf_raw_html_path,
            sections: sections,
            pdf_info: HashMap::new(),
            use_llm: true,
            math_texts: HashMap::new(),
            extract_references: false,
            references: Vec::new(),
            dropped_texts: Vec::new(),
            source_char_count: 0,
        }
    }

    /// Returns the width of the PDF page.
    ///
    /// This function retrieves the width of the PDF page from the `pdf_info` field,
    /// which is a `HashMap` containing additional information about the PDF.
    ///
    /// # Returns
    ///
    /// A `Result<i32>` representing the width of the PDF page.
    ///
    /// # Errors
    ///
    /// Returns an error if the `page_width` key is not found in the `pdf_info`
    /// `HashMap` or if the value cannot be parsed as an `i32`.
    pub fn pdf_width(&self) -> anyhow::Result<i32> {
        self.pdf_info
            .get("page_width")
            .ok_or_else(|| anyhow::anyhow!("PDF width not available - pdfinfo may have failed"))?
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("Invalid page_width value: {}", e))
    }

    /// Returns the height of the PDF page.
    ///
    /// This function retrieves the height of the PDF page from the `pdf_info` field,
    /// which is a `HashMap` containing additional information about the PDF.
    ///
    /// # Returns
    ///
    /// A `Result<i32>` representing the height of the PDF page.
    ///
    /// # Errors
    ///
    /// Returns an error if the `page_height` key is not found in the `pdf_info`
    /// `HashMap` or if the value cannot be parsed as an `i32`.
    pub fn pdf_height(&self) -> anyhow::Result<i32> {
        self.pdf_info
            .get("page_height")
            .ok_or_else(|| anyhow::anyhow!("PDF height not available - pdfinfo may have failed"))?
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("Invalid page_height value: {}", e))
    }

    /// Clears everything a previous `parse` derived from a document, keeping the
    /// settings that describe *how* to parse.
    ///
    /// `ParserConfig` doubles as settings and as the place results are reported, so a
    /// config reused for a second document would otherwise mix the two documents:
    /// `sections` and `dropped_texts` are appended to rather than replaced, and
    /// `pdf_figures` keeps entries for pages the shorter document does not have.
    ///
    /// Settings (`pdf_path`, `pdf_text_path`, `pdf_xml_path`, `use_llm`,
    /// `extract_references`) are deliberately left alone.
    pub fn reset_document_state(&mut self) {
        self.pdf_figures.clear();
        self.pdf_info.clear();
        self.sections.clear();
        self.math_texts.clear();
        self.references.clear();
        self.dropped_texts.clear();
        self.source_char_count = 0;
    }

    /// Records a fragment of text that a filter removed.
    ///
    /// Blank fragments are ignored — they carry no content and would only add noise.
    pub(crate) fn record_drop(&mut self, page: PageNumber, reason: DropReason, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        tracing::debug!("Dropped from page {} ({}): {}", page, reason, text.trim());
        self.dropped_texts.push(DroppedText {
            page,
            reason,
            text: text.to_string(),
        });
    }

    /// Normalized character count of everything the filters discarded.
    pub fn dropped_char_count(&self) -> usize {
        self.dropped_texts.iter().map(|d| normalized_len(&d.text)).sum()
    }

    /// Fraction of the source text that survived into the parsed pages, in `0.0..=1.0`.
    ///
    /// Returns `None` before parsing has run (when `source_char_count` is still 0).
    /// This is a *lower bound* on fidelity: it counts characters, so it cannot tell
    /// reordered text from correct text.
    pub fn coverage(&self) -> Option<f32> {
        if self.source_char_count == 0 {
            return None;
        }
        let dropped = self.dropped_char_count().min(self.source_char_count);
        Some(1.0 - dropped as f32 / self.source_char_count as f32)
    }

    /// Cleans up the generated files associated with the `ParserConfig` instance.
    ///
    /// This function removes the following files if they exist:
    /// - The PDF file at `pdf_path`.
    /// - The HTML text version of the PDF at `pdf_text_path`.
    /// - The raw XML version of the PDF at `pdf_xml_path`.
    /// - Any files associated with figures stored in the `pdf_figures` `HashMap`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating the success or failure of the file removal operations.
    ///
    /// # Errors
    ///
    /// This function will return an error if any of the file removal operations fail.
    pub fn clean_files(&self) -> Result<()> {
        if Path::new(&self.pdf_path).exists() {
            std::fs::remove_file(&self.pdf_path)?;
        }
        if Path::new(&self.pdf_text_path).exists() {
            std::fs::remove_file(&self.pdf_text_path)?;
        }
        if Path::new(&self.pdf_xml_path).exists() {
            std::fs::remove_file(&self.pdf_xml_path)?;
        }
        for figure in self.pdf_figures.values() {
            if Path::new(figure).exists() {
                std::fs::remove_file(figure)?;
            }
        }
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_is_none_before_parsing() {
        assert!(ParserConfig::new().coverage().is_none());
    }

    #[test]
    fn test_coverage_counts_only_alphanumerics() {
        let mut config = ParserConfig::new();
        config.source_char_count = 100;
        // 8 alphanumerics; the punctuation and spaces do not count.
        config.record_drop(1, DropReason::TableRegion, "a b, c d! ef gh");
        assert_eq!(config.dropped_char_count(), 8);
        assert_eq!(config.coverage(), Some(0.92));
    }

    #[test]
    fn test_blank_drops_are_not_recorded() {
        let mut config = ParserConfig::new();
        config.record_drop(1, DropReason::NarrowBlock, "   \n ");
        assert!(config.dropped_texts.is_empty());
    }

    #[test]
    fn test_reset_document_state_clears_everything_derived_from_a_document() {
        let mut config = ParserConfig::new();
        config.source_char_count = 500;
        config.record_drop(1, DropReason::TableRegion, "55%");
        config.sections.push((1, "Abstract".to_string()));
        config.pdf_figures.insert(1, "/tmp/page-1.png".to_string());
        config.pdf_info.insert("page_width".to_string(), "595".to_string());
        config.math_texts.insert((1, 0), "<math>x</math>".to_string());

        config.reset_document_state();

        assert!(config.dropped_texts.is_empty());
        assert_eq!(config.source_char_count, 0);
        assert!(config.sections.is_empty());
        assert!(config.pdf_figures.is_empty());
        assert!(config.pdf_info.is_empty());
        assert!(config.math_texts.is_empty());
        assert!(config.references.is_empty());
        assert!(
            config.coverage().is_none(),
            "coverage has no meaning before the next parse"
        );
    }

    #[test]
    fn test_reset_document_state_keeps_settings() {
        // Settings describe how to parse, not what was parsed, so they must survive.
        let mut config = ParserConfig::new();
        config.use_llm = false;
        config.extract_references = true;
        let pdf_path = config.pdf_path.clone();
        let text_path = config.pdf_text_path.clone();
        let xml_path = config.pdf_xml_path.clone();

        config.reset_document_state();

        assert!(!config.use_llm);
        assert!(config.extract_references);
        assert_eq!(config.pdf_path, pdf_path);
        assert_eq!(config.pdf_text_path, text_path);
        assert_eq!(config.pdf_xml_path, xml_path);
    }

    #[test]
    fn test_coverage_is_not_polluted_by_a_previous_document() {
        let mut config = ParserConfig::new();
        // First document: 50 of 100 characters discarded.
        config.source_char_count = 100;
        config.record_drop(1, DropReason::TableRegion, &"a".repeat(50));
        assert_eq!(config.coverage(), Some(0.5));

        // Second document, same config: nothing discarded, so coverage is 100%.
        config.reset_document_state();
        config.source_char_count = 100;
        assert_eq!(config.coverage(), Some(1.0));
    }

    #[test]
    fn test_coverage_is_clamped_when_drops_exceed_source() {
        // Defensive: a caller could set an implausible source count. Coverage must
        // stay in 0.0..=1.0 rather than going negative.
        let mut config = ParserConfig::new();
        config.source_char_count = 2;
        config.record_drop(1, DropReason::OutsideTextArea, "abcdef");
        assert_eq!(config.coverage(), Some(0.0));
    }
}
