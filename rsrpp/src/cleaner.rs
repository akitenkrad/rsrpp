//! Text cleaning and block classification module.
//!
//! This module provides functionality for:
//! - Detecting and classifying figure/table captions
//! - Classifying blocks by type (Body, Caption, Header)

use regex::Regex;
use std::sync::LazyLock;

use crate::models::{Block, BlockType, Line, Page};

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

/// Horizontal slack, as a share of the font size, between the left edge of a caption's
/// first line and that of a line that continues it. A caption is set as a justified
/// paragraph, so its lines all start at the same margin; measured over the caption
/// blocks of seven arXiv papers the deviation is 0.03 font sizes at the median and
/// never reaches 0.5. A table cell in the same block starts at its column instead.
const CAPTION_LEFT_TOLERANCE: f32 = 0.5;

/// How far the vertical step between two lines may stray from the step the caption
/// established, as a fraction of that step, before the second line is read as something
/// other than more caption. Caption leading is uniform: across the same seven papers no
/// caption block varies by more than 16%.
const CAPTION_STEP_TOLERANCE: f32 = 0.5;

/// Widest step between two lines, as a multiple of the font size, that still reads as
/// the next line of the same paragraph rather than the start of something else.
///
/// This is what the step tolerance above cannot judge: the first line to follow the
/// caption has no established leading to be compared against, so without an absolute
/// bound it is accepted however far below the caption it sits — and a one-line caption
/// printed above a table would hand the first row to the body text. Across the caption
/// blocks of seven arXiv papers the step is 1.34 font sizes at the median and never
/// reaches 1.60.
const CAPTION_MAX_LEADING: f32 = 2.0;

/// The font size the caption is set in, taken as the median word height of its first line
/// that has any words.
///
/// Measured on the caption itself, never on the whole block: where poppler has merged a
/// table into the block, the cells would otherwise set the scale that the caption is then
/// judged against. One line rather than two for the same reason — the second line of the
/// block may already be a table row, and letting it set the scale is exactly what the
/// bounds below exist to prevent. A line with no words has no size to offer and is passed
/// over; a block of nothing but those returns 0, which the caller reads as "no scale to
/// judge by" rather than as a bound of zero.
fn caption_font_size(block: &Block) -> f32 {
    let Some(line) = block.lines.iter().find(|line| !line.words.is_empty()) else {
        return 0.0;
    };
    let mut heights: Vec<f32> = line.words.iter().map(|word| word.height).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    heights[heights.len() / 2]
}

/// How many of `block`'s leading lines belong to its caption, or 0 if it has no caption.
///
/// Poppler decides block boundaries by geometry, so a caption printed close above its
/// table can arrive in the same block as the table's rows. Everything the caption covers
/// has to be protected from the table-region filter — a caption rarely fits on one line,
/// and half a caption is not much better than none — but protecting the whole block
/// would carry the rows along with it and leak them into the body text.
///
/// The caption is the run of lines that keeps the typography the first line set: stacked
/// one below the next, evenly spaced, starting at the same left margin. Table rows break
/// all three — cells of one row share a baseline, and a row starts at its column.
pub fn caption_line_span(block: &Block) -> usize {
    if !is_caption(block) || block.lines.is_empty() {
        return 0;
    }

    let font_size = caption_font_size(block);
    let left = block.lines[0].x;
    let mut leading: Option<f32> = None;
    let mut span = 1;

    for pair in block.lines.windows(2) {
        let (previous, line) = (&pair[0], &pair[1]);
        let step = line.y - previous.y;
        // Lines that share a baseline sit side by side, which caption text never does,
        // and a line a paragraph's width below is no longer the same paragraph. With no
        // font size to scale them by, the two bounds would read as zero and cut every
        // caption to its first line, so they stand down and the leading decides alone.
        if step <= 0.0 {
            break;
        }
        if font_size > 0.0 {
            if step > CAPTION_MAX_LEADING * font_size {
                break;
            }
            if (line.x - left).abs() > CAPTION_LEFT_TOLERANCE * font_size {
                break;
            }
        }
        match leading {
            None => leading = Some(step),
            Some(expected) => {
                if (step - expected).abs() > CAPTION_STEP_TOLERANCE * expected {
                    break;
                }
            }
        }
        span += 1;
    }

    span
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

    fn block_with_lines(lines: &[(f32, f32, &str)]) -> Block {
        let mut block = Block::new(0.0, 0.0, 400.0, 100.0);
        for (y, x, text) in lines {
            let mut line = Line::new(*x, *y, 200.0, 10.0);
            for word in text.split_whitespace() {
                line.add_word(word.to_string(), *x, *y, 10.0, 10.0);
            }
            block.lines.push(line);
        }
        block
    }

    #[test]
    fn test_caption_line_span_covers_a_whole_caption() {
        // Caption lines are stacked, evenly spaced and share a left margin, however
        // short the last one is — cutting it would leave half a caption.
        let block = block_with_lines(&[
            (
                100.0,
                72.0,
                "Table 4: The Transformer generalizes well to parsing",
            ),
            (
                111.0,
                72.0,
                "with the settings of Section 3, trained on WSJ only",
            ),
            (122.0, 72.2, "(Results are on Section 23)"),
        ]);
        assert_eq!(caption_line_span(&block), 3);
    }

    #[test]
    fn test_caption_line_span_stops_at_the_table() {
        // The rows below the caption start at their columns and, within a row, share a
        // top. Either break ends the caption.
        let block = block_with_lines(&[
            (100.0, 72.0, "Table 4: The Transformer generalizes well"),
            (111.0, 72.0, "(Results are on Section 23 of WSJ)"),
            (123.0, 200.0, "Parser"),
            (123.0, 330.0, "WSJ 23 F1"),
        ]);
        assert_eq!(caption_line_span(&block), 2);
    }

    #[test]
    fn test_caption_line_span_stops_below_a_one_line_caption() {
        // A one-line caption gives the next line no leading to be measured against, so
        // an absolute bound is all that keeps a table header printed below it — same
        // margin, but a paragraph's width away — from being read as more caption.
        let block = block_with_lines(&[
            (100.0, 72.0, "Table 1: Results"),
            (160.0, 72.0, "Model"),
            (160.0, 200.0, "Accuracy"),
        ]);
        assert_eq!(caption_line_span(&block), 1);
    }

    #[test]
    fn test_caption_line_span_measures_the_captions_own_font() {
        // The table merged into the block is set larger than the caption. Scaling the
        // tolerances to it would slacken them for text they are not measuring.
        let mut block = block_with_lines(&[(100.0, 72.0, "Table 1: Results")]);
        // Set larger, and in enough words to carry the median of the whole block. Judged
        // by that median the first row is within a caption's leading of the caption;
        // judged by the caption's own 10pt it is half a page away.
        for (y, x, text) in [
            (130.0, 74.0, "Model A scored 91.2 on average"),
            (170.0, 74.0, "Model B scored 89.7 on average"),
        ] {
            let mut line = Line::new(x, y, 200.0, 24.0);
            for word in text.split_whitespace() {
                line.add_word(word.to_string(), x, y, 24.0, 24.0);
            }
            block.lines.push(line);
        }
        assert_eq!(caption_line_span(&block), 1);
    }

    #[test]
    fn test_caption_line_span_stops_where_the_leading_changes() {
        // Same margin, but the third line sits a row height away rather than a line
        // height: it belongs to whatever follows the caption, not to the caption.
        let block = block_with_lines(&[
            (100.0, 72.0, "Figure 2: Accuracy against context length"),
            (111.0, 72.0, "on the held-out split."),
            (140.0, 72.0, "Model A"),
        ]);
        assert_eq!(caption_line_span(&block), 2);
    }

    #[test]
    fn test_caption_line_span_survives_a_first_line_without_words() {
        // A line with no words has no font size. Reading zero out of it and scaling the
        // bounds by it would cut every such caption to a single line.
        let mut block = Block::new(0.0, 0.0, 400.0, 100.0);
        block.lines.push(Line::new(72.0, 100.0, 200.0, 0.0));
        for (y, text) in [
            (111.0, "Table 2: Accuracy by model on the held-out split"),
            (122.0, "averaged over five seeds."),
        ] {
            let mut line = Line::new(72.0, y, 200.0, 10.0);
            for word in text.split_whitespace() {
                line.add_word(word.to_string(), 72.0, y, 10.0, 10.0);
            }
            block.lines.push(line);
        }
        assert_eq!(caption_line_span(&block), 3);
    }

    #[test]
    fn test_caption_line_span_is_zero_without_a_caption() {
        let block = block_with_lines(&[(100.0, 72.0, "The results are shown below.")]);
        assert_eq!(caption_line_span(&block), 0);
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
