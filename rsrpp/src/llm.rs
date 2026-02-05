use anyhow::Result;
use openai_tools::chat::request::ChatCompletion;
use openai_tools::common::message::{Content, Message};
use openai_tools::common::role::Role;
use regex::Regex;
use std::sync::LazyLock;

use crate::config::PageNumber;
use crate::models::Page;

/// Regex pattern for Unicode math symbols (Greek letters, operators, arrows, etc.)
static MATH_CHAR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"[\u{2200}-\u{22FF}\u{2A00}-\u{2AFF}\u{03B1}-\u{03C9}\u{0391}-\u{03A9}∑∫∏√∞±×÷≠≤≥≈∈∉⊂⊃∪∩→←↔⇒⇐⇔]",
    )
    .unwrap()
});

/// Regex pattern for math-like structures (equations, subscripts, superscripts)
static MATH_STRUCTURE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        # Variable = expression pattern
        [a-zA-Z]\s*[=<>≤≥≈]\s*[a-zA-Z0-9]
        |
        # Subscript/superscript Unicode characters
        [⁰¹²³⁴⁵⁶⁷⁸⁹ⁿ⁺⁻₀₁₂₃₄₅₆₇₈₉ₙ]
        |
        # Fraction-like patterns
        \d+\s*/\s*\d+
        |
        # Function notation f(x), g(y), etc.
        [a-zA-Z]\s*\(\s*[a-zA-Z]\s*\)
        |
        # Summation/product notation patterns
        \bsum\b|\bprod\b|\bint\b|\blim\b
    ",
    )
    .unwrap()
});

/// Regex pattern for inline math expression boundaries
static INLINE_MATH_BOUNDARY: LazyLock<Regex> = LazyLock::new(|| {
    // Matches sequences containing math symbols/structures surrounded by word boundaries
    Regex::new(
        r"(?x)
        # Sequence with math symbols and alphanumeric characters
        (?:
            (?:[a-zA-Z0-9]+\s*)?
            [\u{2200}-\u{22FF}\u{2A00}-\u{2AFF}\u{03B1}-\u{03C9}\u{0391}-\u{03A9}=<>≤≥≈∑∫∏√∞±×÷≠∈∉⊂⊃∪∩→←↔⇒⇐⇔\^_{}()\[\]]
            (?:\s*[a-zA-Z0-9\^_{}()\[\]]+)*
        )+
    ",
    )
    .unwrap()
});

const MATH_EXTRACTION_PROMPT: &str = r#"You are an academic paper text extractor. Extract ALL text from this page image.

Rules:
- Render inline math as $...$ (LaTeX)
- Render display math as $$...$$ (LaTeX)
- Preserve paragraph structure with blank lines
- Keep section titles, figure captions, and table headers as-is
- Output plain text (no markdown headers, no bullet formatting)"#;

const SECTION_EXTRACTION_PROMPT: &str = r#"You are an academic paper analyzer. Look at these pages from a research paper and extract the complete section structure.

Return a JSON array of section titles in document order:
["Abstract", "Introduction", "Related Work", ...]

Include ALL sections from the paper, including appendices.
Only return top-level section titles (not subsections like "2.1 ...").
Return ONLY the JSON array, no other text."#;

/// Check if LLM processing is available (OPENAI_API_KEY is set)
pub fn is_llm_available() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

/// Estimate math density in a page's text.
/// Returns a score from 0.0 to 1.0 indicating the likelihood of math content.
pub fn estimate_math_density(page_text: &str) -> f32 {
    if page_text.is_empty() {
        return 0.0;
    }

    let total_chars = page_text.chars().count() as f32;
    let mut score = 0.0f32;

    // Unicode math symbols
    let math_symbols = regex::Regex::new(
        r"[\u{2200}-\u{22FF}\u{2190}-\u{21FF}\u{2A00}-\u{2AFF}\u{00B1}\u{00D7}\u{00F7}]",
    )
    .unwrap();
    let math_count = math_symbols.find_iter(page_text).count() as f32;
    score += (math_count / total_chars * 50.0).min(0.3);

    // Single-character token sequences (e.g., "f ( x ) = ...")
    let single_char_seq = regex::Regex::new(r"(\b\w\b\s){3,}").unwrap();
    let seq_count = single_char_seq.find_iter(page_text).count() as f32;
    score += (seq_count / total_chars * 100.0).min(0.3);

    // Fraction-like patterns
    let fraction_ptn = regex::Regex::new(r"\d+\s*/\s*\d+").unwrap();
    let frac_count = fraction_ptn.find_iter(page_text).count() as f32;
    score += (frac_count / total_chars * 100.0).min(0.2);

    // Subscript/superscript unicode characters
    let sub_super = regex::Regex::new(r"[\u{2080}-\u{2089}\u{2070}-\u{2079}]").unwrap();
    let ss_count = sub_super.find_iter(page_text).count() as f32;
    score += (ss_count / total_chars * 100.0).min(0.2);

    score.min(1.0)
}

/// Extract text (including math formulas in LaTeX) from a page image using GPT-4o Vision.
pub async fn extract_page_text_with_math(
    image_path: &str,
    _page_number: PageNumber,
) -> Result<String> {
    let contents = vec![
        Content::from_text(MATH_EXTRACTION_PROMPT),
        Content::from_image_file(image_path),
    ];
    let message = Message::from_message_array(Role::User, contents);

    let mut chat = ChatCompletion::new();
    let response = chat
        .model_id("gpt-4o")
        .messages(vec![message])
        .temperature(0.0)
        .chat()
        .await
        .map_err(|e| anyhow::anyhow!("LLM API call failed: {}", e))?;

    let text = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .and_then(|c| c.text.as_ref())
        .map(|t| t.to_string())
        .unwrap_or_default();

    Ok(text)
}

/// Validate and extract section structure from page images using GPT-4o Vision.
/// Sends the first few pages to detect the paper's section structure.
pub async fn validate_sections(first_pages_images: &[String]) -> Result<Vec<String>> {
    if first_pages_images.is_empty() {
        return Ok(Vec::new());
    }

    let mut contents = vec![Content::from_text(SECTION_EXTRACTION_PROMPT)];
    for image_path in first_pages_images {
        contents.push(Content::from_image_file(image_path));
    }
    let message = Message::from_message_array(Role::User, contents);

    let mut chat = ChatCompletion::new();
    let response = chat
        .model_id("gpt-4o")
        .messages(vec![message])
        .temperature(0.0)
        .chat()
        .await
        .map_err(|e| anyhow::anyhow!("LLM section validation failed: {}", e))?;

    let text = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .and_then(|c| c.text.as_ref())
        .map(|t| t.to_string())
        .unwrap_or_default();

    // Parse JSON array from response
    let sections: Vec<String> = serde_json::from_str(&text).unwrap_or_else(|_| {
        // Try to extract JSON array from the response text
        if let Some(start) = text.find('[') {
            if let Some(end) = text.rfind(']') {
                serde_json::from_str(&text[start..=end]).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    });

    Ok(sections)
}

/// Merge font-based section detection results with LLM-validated sections.
///
/// Strategy:
/// - LLM results are treated as ground truth for section names
/// - Page numbers are inherited from font-based results where matching
/// - Sections found by LLM but not by font-based detection get page_number = -1
///   (matched by text only in parse_extract_section_text)
/// - Sections found by font-based but not LLM are excluded (likely false positives)
pub fn merge_sections(
    font_based: &[(PageNumber, String)],
    llm_sections: &[String],
) -> Vec<(PageNumber, String)> {
    let mut merged: Vec<(PageNumber, String)> = Vec::new();

    for llm_section in llm_sections {
        let llm_lower = llm_section.to_lowercase();

        // Find matching font-based section
        let font_match = font_based.iter().find(|(_, name)| name.to_lowercase() == llm_lower);

        if let Some((page, _)) = font_match {
            merged.push((*page, llm_section.clone()));
        } else {
            // LLM found it but font-based didn't → add with page = -1 for text-only matching
            merged.push((-1, llm_section.clone()));
        }
    }

    merged
}

/// Check if a text segment contains math-like content.
///
/// # Arguments
///
/// * `text` - The text segment to analyze.
///
/// # Returns
///
/// `true` if the text contains math symbols or structures.
pub fn contains_math(text: &str) -> bool {
    MATH_CHAR_PATTERN.is_match(text) || MATH_STRUCTURE_PATTERN.is_match(text)
}

/// Apply heuristic math markup to text using `<math>...</math>` tags.
///
/// This function detects inline math expressions and wraps them with math tags.
/// It uses pattern matching to identify:
/// - Unicode math symbols (Greek letters, operators, etc.)
/// - Equation-like structures (a = b, f(x), etc.)
/// - Fraction patterns (1/2, etc.)
///
/// # Arguments
///
/// * `text` - The text to process.
///
/// # Returns
///
/// The text with math expressions wrapped in `<math>...</math>` tags.
pub fn mark_math_heuristic(text: &str) -> String {
    if text.is_empty() || !contains_math(text) {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len() * 2);
    let mut last_end = 0;

    // Find all math-like segments
    for mat in INLINE_MATH_BOUNDARY.find_iter(text) {
        let start = mat.start();
        let end = mat.end();
        let matched = mat.as_str().trim();

        // Skip very short matches or matches that are just numbers
        if matched.len() < 2 || matched.chars().all(|c| c.is_ascii_digit()) {
            result.push_str(&text[last_end..end]);
            last_end = end;
            continue;
        }

        // Only mark if it contains actual math content
        if MATH_CHAR_PATTERN.is_match(matched) || MATH_STRUCTURE_PATTERN.is_match(matched) {
            // Add text before this match
            result.push_str(&text[last_end..start]);
            // Wrap match in math tags
            result.push_str("<math>");
            result.push_str(matched);
            result.push_str("</math>");
        } else {
            // No math content, keep as-is
            result.push_str(&text[last_end..end]);
        }
        last_end = end;
    }

    // Add remaining text
    result.push_str(&text[last_end..]);
    result
}

/// Apply heuristic math markup to all blocks in pages.
///
/// This function iterates through all blocks and creates math-marked versions
/// of their text content.
///
/// # Arguments
///
/// * `pages` - A mutable reference to the pages to process.
///
/// # Returns
///
/// A map of (page_number, block_index) to math-marked text.
pub fn apply_heuristic_math_markup(
    pages: &[Page],
) -> std::collections::HashMap<(PageNumber, usize), String> {
    let mut math_texts: std::collections::HashMap<(PageNumber, usize), String> =
        std::collections::HashMap::new();

    for page in pages {
        for (block_idx, block) in page.blocks.iter().enumerate() {
            let text = block.get_text();
            let marked = mark_math_heuristic(&text);

            // Only store if math was actually marked
            if marked != text {
                math_texts.insert((page.page_number, block_idx), marked);
            }
        }
    }

    math_texts
}

/// Convert LLM LaTeX math notation ($...$, $$...$$) to our custom math tags.
///
/// # Arguments
///
/// * `text` - Text containing LaTeX-style math notation.
///
/// # Returns
///
/// Text with math notation converted to `<math>...</math>` tags.
pub fn convert_latex_to_math_tags(text: &str) -> String {
    // Convert display math first ($$...$$)
    let display_re = Regex::new(r"\$\$([^$]+)\$\$").unwrap();
    let result = display_re.replace_all(text, r#"<math display="block">$1</math>"#);

    // Then convert inline math ($...$)
    let inline_re = Regex::new(r"\$([^$]+)\$").unwrap();
    let result = inline_re.replace_all(&result, "<math>$1</math>");

    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_math() {
        // Text with Greek letters
        assert!(contains_math(
            "The variable α represents the learning rate."
        ));
        // Text with operators
        assert!(contains_math("We have a ≤ b and x ∈ S."));
        // Equation pattern
        assert!(contains_math("Given f(x) = ax + b"));
        // Fraction
        assert!(contains_math("The ratio is 1/2."));
        // No math
        assert!(!contains_math("This is a normal sentence."));
    }

    #[test]
    fn test_mark_math_heuristic_greek() {
        let text = "The variable α represents the learning rate.";
        let marked = mark_math_heuristic(text);
        assert!(marked.contains("<math>"));
        assert!(marked.contains("α"));
        assert!(marked.contains("</math>"));
    }

    #[test]
    fn test_mark_math_heuristic_operators() {
        let text = "We have a ≤ b.";
        let marked = mark_math_heuristic(text);
        assert!(marked.contains("<math>"));
    }

    #[test]
    fn test_mark_math_heuristic_no_math() {
        let text = "This is a normal sentence with no math.";
        let marked = mark_math_heuristic(text);
        assert_eq!(marked, text);
        assert!(!marked.contains("<math>"));
    }

    #[test]
    fn test_convert_latex_to_math_tags_inline() {
        let text = "The equation $f(x) = ax^2$ represents a parabola.";
        let converted = convert_latex_to_math_tags(text);
        assert!(converted.contains("<math>f(x) = ax^2</math>"));
        assert!(!converted.contains("$"));
    }

    #[test]
    fn test_convert_latex_to_math_tags_display() {
        let text = "The equation is:\n$$\\sum_{i=1}^{n} x_i$$\nwhich represents...";
        let converted = convert_latex_to_math_tags(text);
        assert!(converted.contains(r#"<math display="block">"#));
        assert!(converted.contains("</math>"));
        assert!(!converted.contains("$$"));
    }

    #[test]
    fn test_convert_latex_to_math_tags_mixed() {
        let text = "Inline $a$ and display $$b$$";
        let converted = convert_latex_to_math_tags(text);
        assert!(converted.contains("<math>a</math>"));
        assert!(converted.contains(r#"<math display="block">b</math>"#));
    }
}
