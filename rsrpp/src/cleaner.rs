//! Text cleaning and block classification module.
//!
//! This module provides functionality for:
//! - Detecting and classifying figure/table captions
//! - Classifying blocks by type (Body, Caption, Header)

use regex::Regex;
use std::sync::LazyLock;

use crate::models::{Block, BlockType, Page};

/// Pre-compiled regex patterns for caption detection.
/// Matches patterns like:
/// - "Figure 1:", "Fig. 2.", "FIGURE 3:"
/// - "Table 1:", "TABLE 2."
/// - "Scheme 1:", "Algorithm 2:"
/// - "Listing 1:"
static CAPTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Figure patterns: "Figure 1:", "Fig. 2.", "FIG 3:", etc.
        Regex::new(r"(?i)^(?:fig(?:ure)?\.?\s*\d+[.:]?)").unwrap(),
        // Table patterns: "Table 1:", "TABLE 2.", etc.
        Regex::new(r"(?i)^(?:table\.?\s*\d+[.:]?)").unwrap(),
        // Scheme/Algorithm patterns
        Regex::new(r"(?i)^(?:scheme|algorithm)\.?\s*\d+[.:]?").unwrap(),
        // Listing patterns (for code listings)
        Regex::new(r"(?i)^(?:listing)\.?\s*\d+[.:]?").unwrap(),
        // Appendix figure/table patterns: "Appendix Figure A1:"
        Regex::new(r"(?i)^(?:appendix\s+)?(?:fig(?:ure)?|table)\.?\s*[A-Za-z]?\d+[.:]?").unwrap(),
    ]
});

/// Checks if a block's text matches a caption pattern.
///
/// # Arguments
///
/// * `block` - A reference to the Block to check.
///
/// # Returns
///
/// `true` if the block text starts with a caption pattern (e.g., "Figure 1:", "Table 2.").
pub fn is_caption(block: &Block) -> bool {
    let text = block.get_text();
    let trimmed = text.trim();
    CAPTION_PATTERNS.iter().any(|re| re.is_match(trimmed))
}

/// Classifies blocks in pages by their type (Body, Caption, Header).
///
/// This function iterates through all blocks in the provided pages and
/// sets the `block_type` field based on content analysis:
/// - Blocks matching caption patterns are marked as `BlockType::Caption`
/// - Other blocks remain as `BlockType::Body` (default)
///
/// # Arguments
///
/// * `pages` - A mutable reference to a vector of Pages to classify.
pub fn classify_blocks(pages: &mut Vec<Page>) {
    for page in pages.iter_mut() {
        for block in page.blocks.iter_mut() {
            if is_caption(block) {
                block.block_type = BlockType::Caption;
            }
            // Future: Add header detection logic here
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Block, Line, Word};

    fn make_block_with_text(text: &str) -> Block {
        let mut block = Block::new(0.0, 0.0, 100.0, 20.0);
        let mut line = Line::new(0.0, 0.0, 100.0, 20.0);
        line.words.push(Word {
            text: text.to_string(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 20.0,
        });
        block.lines.push(line);
        block
    }

    #[test]
    fn test_is_caption_figure_patterns() {
        // Basic figure patterns
        assert!(is_caption(&make_block_with_text(
            "Figure 1: Overview of the system"
        )));
        assert!(is_caption(&make_block_with_text(
            "Figure 1. Overview of the system"
        )));
        assert!(is_caption(&make_block_with_text("Fig. 1: Overview")));
        assert!(is_caption(&make_block_with_text("Fig 2. Architecture")));
        assert!(is_caption(&make_block_with_text("FIGURE 3: Results")));
        assert!(is_caption(&make_block_with_text("FIG. 4. Comparison")));
    }

    #[test]
    fn test_is_caption_table_patterns() {
        assert!(is_caption(&make_block_with_text(
            "Table 1: Performance metrics"
        )));
        assert!(is_caption(&make_block_with_text(
            "Table 2. Comparison results"
        )));
        assert!(is_caption(&make_block_with_text("TABLE 3: Summary")));
    }

    #[test]
    fn test_is_caption_other_patterns() {
        assert!(is_caption(&make_block_with_text(
            "Algorithm 1: Main procedure"
        )));
        assert!(is_caption(&make_block_with_text(
            "Scheme 2. Reaction pathway"
        )));
        assert!(is_caption(&make_block_with_text(
            "Listing 1: Python code example"
        )));
    }

    #[test]
    fn test_is_caption_appendix_patterns() {
        assert!(is_caption(&make_block_with_text(
            "Appendix Figure A1: Additional results"
        )));
        assert!(is_caption(&make_block_with_text(
            "Figure A1: Supplementary data"
        )));
        assert!(is_caption(&make_block_with_text(
            "Table B2. Extended metrics"
        )));
    }

    #[test]
    fn test_is_caption_non_captions() {
        // Normal body text should not be detected as captions
        assert!(!is_caption(&make_block_with_text(
            "This is a regular paragraph."
        )));
        assert!(!is_caption(&make_block_with_text(
            "The figure shows the results."
        )));
        assert!(!is_caption(&make_block_with_text(
            "As shown in Table 1, the results..."
        )));
        assert!(!is_caption(&make_block_with_text(
            "See Figure 1 for details."
        )));
        assert!(!is_caption(&make_block_with_text("1. Introduction")));
        assert!(!is_caption(&make_block_with_text("Abstract")));
    }

    #[test]
    fn test_classify_blocks() {
        let mut pages = vec![Page::new(612.0, 792.0, 1)];

        // Add a body block
        let body_block = make_block_with_text("This is normal text.");
        pages[0].blocks.push(body_block);

        // Add a caption block
        let caption_block = make_block_with_text("Figure 1: System overview");
        pages[0].blocks.push(caption_block);

        // Add another body block
        let body_block2 = make_block_with_text("More normal text here.");
        pages[0].blocks.push(body_block2);

        classify_blocks(&mut pages);

        assert_eq!(pages[0].blocks[0].block_type, BlockType::Body);
        assert_eq!(pages[0].blocks[1].block_type, BlockType::Caption);
        assert_eq!(pages[0].blocks[2].block_type, BlockType::Body);
    }
}
