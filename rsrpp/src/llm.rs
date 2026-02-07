use anyhow::Result;
use openai_tools::chat::request::ChatCompletion;
use openai_tools::common::message::{Content, Message};
use openai_tools::common::role::Role;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::config::PageNumber;
use crate::models::{Page, Reference};

/// Regex pattern for Unicode math symbols (Greek letters, operators, arrows, etc.)
static MATH_CHAR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"[\u{2200}-\u{22FF}\u{2A00}-\u{2AFF}\u{03B1}-\u{03C9}\u{0391}-\u{03A9}\u{2032}-\u{2037}\u{207A}-\u{207E}\u{2080}-\u{208E}∑∫∏√∞±×÷≠≤≥≈∈∉⊂⊃∪∩→←↔⇒⇐⇔]",
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
        # Fraction-like patterns (exclude dates by limiting to 1-3 digits)
        \b\d{1,3}\s*/\s*\d{1,3}\b
        |
        # Function notation f(x), g(y) - lowercase only to avoid false positives
        [a-z]\s*\(\s*[a-z]\s*\)
        |
        # Summation/product notation patterns (removed \bint\b - conflicts with integer)
        \bsum\b|\bprod\b|\blim\b
        |
        # Multi-character math functions
        \b(?:sin|cos|tan|cot|sec|csc|arcsin|arccos|arctan|sinh|cosh|tanh|log|ln|exp|det|dim|ker|min|max|sup|inf|arg|sgn|diag|tr|rank)\s*[(\[]
        |
        # ASCII exponents: x^2, e^{iπ}
        [a-zA-Z)}\]]\s*\^\s*(?:\{[^}]+\}|\d+|[a-zA-Z])
        |
        # ASCII subscripts: x_i, a_{n+1}
        [a-zA-Z)}\]]\s*_\s*(?:\{[^}]+\}|\d+|[a-zA-Z])
        |
        # Letter fractions: a/b (single letter on both sides of /)
        \b[a-zA-Z]\s*/\s*[a-zA-Z]\b
        |
        # Norm notation: ||x||, ||w||_2
        \|\|[^|]+\|\|
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
            [\u{2200}-\u{22FF}\u{2A00}-\u{2AFF}\u{03B1}-\u{03C9}\u{0391}-\u{03A9}=<>≤≥≈∑∫∏√∞±×÷≠∈∉⊂⊃∪∩→←↔⇒⇐⇔|/^_{}()\[\]]
            (?:\s*[a-zA-Z0-9\^_{}()\[\]]+)*
        )+
    ",
    )
    .unwrap()
});

/// Regex pattern for common false positive math detections
static MATH_FALSE_POSITIVE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?x)
        # Date patterns: 2019/2020, 01/15/2023
        \b\d{2,4}\s*/\s*\d{2,4}\b
        |
        # Statistical reporting: n = 50 participants, p < 0.05
        \b[nNpPtTkK]\s*[=<>]\s*\d+(?:\.\d+)?\s*(?:participants|subjects|samples|items|trials|percent|%)
        |
        # Parenthetical references: in (a), method(s), part (i)
        \b(?:in|see|part|method|step|item|case|type|group)\s*\(\s*[a-zA-Z]\s*\)
        |
        # Section references: Section 3.1, Eq. (1), Figure 2
        (?i)(?:section|eq|equation|fig|figure|table|ref|chapter|appendix)\s*[.(\[]\s*\d
    ").unwrap()
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

const REFERENCE_EXTRACTION_PROMPT: &str = r#"You are a bibliographic reference parser. Parse the following References section from an academic paper.

Tasks:
1. Identify each individual reference entry
2. Extract structured fields for each entry

For each reference, extract:
- authors: array of author names (e.g., ["John Smith", "Jane Doe"])
- title: the title of the work
- year: publication year as integer
- venue: journal name, conference name, or publisher
- doi: DOI if present (e.g., "10.1234/example")
- url: URL if present
- arxiv_id: arXiv ID if present (e.g., "2308.10379")
- volume: volume number if present
- pages: page range if present (e.g., "1-15")

Return a JSON array. Use null for missing fields.
Only return the JSON array, no other text.

References section:
"#;

/// Default model to use when OPENAI_API_MODEL is not set
const DEFAULT_MODEL: &str = "gpt-5.2";

/// Check if LLM processing is available (OPENAI_API_KEY is set)
pub fn is_llm_available() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

/// Get the model ID to use for LLM calls.
/// Returns the value of OPENAI_API_MODEL environment variable, or DEFAULT_MODEL if not set.
pub fn get_model_id() -> String {
    std::env::var("OPENAI_API_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
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

/// Extract text (including math formulas in LaTeX) from a page image using LLM Vision.
/// The model is determined by OPENAI_API_MODEL environment variable (default: gpt-5.2).
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
        .model_id(&get_model_id())
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

/// Validate and extract section structure from page images using LLM Vision.
/// Sends the first few pages to detect the paper's section structure.
/// The model is determined by OPENAI_API_MODEL environment variable (default: gpt-5.2).
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
        .model_id(&get_model_id())
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
/// - Font-based sections within the LLM page range are validated against LLM results:
///   confirmed sections are kept, unconfirmed ones are excluded (likely false positives).
/// - Font-based sections outside the LLM page range are preserved as-is,
///   since the LLM never saw those pages and cannot confirm or deny them.
/// - Sections found by LLM but not matched by any font-based section get page_number = -1
///   (matched by text only in parse_extract_section_text).
///
/// # Arguments
///
/// * `font_based` - Sections detected by font analysis, with (page_number, name).
/// * `llm_sections` - Section names returned by the LLM.
/// * `llm_page_range` - Inclusive page range (start, end) that the LLM examined.
pub fn merge_sections(
    font_based: &[(PageNumber, String)],
    llm_sections: &[String],
    llm_page_range: (PageNumber, PageNumber),
) -> Vec<(PageNumber, String)> {
    let mut result: Vec<(PageNumber, String)> = Vec::new();
    let mut used_llm_sections: HashSet<String> = HashSet::new();

    // Step 1: Process font-based sections
    for (page, name) in font_based {
        if *page >= llm_page_range.0 && *page <= llm_page_range.1 {
            // Within LLM range — check if LLM confirmed this section
            if llm_sections.iter().any(|s| s.to_lowercase() == name.to_lowercase()) {
                result.push((*page, name.clone()));
                used_llm_sections.insert(name.to_lowercase());
            }
            // else: LLM didn't confirm — exclude (likely false positive)
        } else {
            // Outside LLM range — trust font-based result
            result.push((*page, name.clone()));
            used_llm_sections.insert(name.to_lowercase());
        }
    }

    // Step 2: Add LLM-only sections (found by LLM but not matched by any font-based)
    for llm_section in llm_sections {
        if !used_llm_sections.contains(&llm_section.to_lowercase()) {
            result.push((-1, llm_section.clone()));
        }
    }

    result
}

/// Extract structured references from a References section using LLM.
///
/// # Arguments
///
/// * `references_text` - The raw text content of the References section.
///
/// # Returns
///
/// A vector of Reference structs with parsed bibliographic fields.
pub async fn extract_references_llm(references_text: &str) -> Result<Vec<Reference>> {
    if references_text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let prompt = format!("{}{}", REFERENCE_EXTRACTION_PROMPT, references_text);
    let message = Message::from_string(Role::User, prompt);

    let mut chat = ChatCompletion::new();
    let response = chat
        .model_id(&get_model_id())
        .messages(vec![message])
        .temperature(0.0)
        .chat()
        .await
        .map_err(|e| anyhow::anyhow!("LLM reference extraction failed: {}", e))?;

    let text = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .and_then(|c| c.text.as_ref())
        .map(|t| t.to_string())
        .unwrap_or_default();

    // Parse the JSON response
    parse_references_json(&text)
}

/// Parse LLM JSON response into Reference structs.
///
/// Handles both clean JSON arrays and responses with extra text around them.
fn parse_references_json(text: &str) -> Result<Vec<Reference>> {
    // First, try to parse as-is
    if let Ok(refs) = serde_json::from_str::<Vec<Reference>>(text) {
        return Ok(refs);
    }

    // Try to extract JSON array from the response
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            let json_str = &text[start..=end];
            if let Ok(refs) = serde_json::from_str::<Vec<Reference>>(json_str) {
                return Ok(refs);
            }
        }
    }

    // If all parsing attempts fail, return empty
    tracing::warn!("Failed to parse LLM reference extraction response");
    Ok(Vec::new())
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

/// Find the nearest valid char boundary at or before `pos`.
fn safe_char_start(text: &str, pos: usize) -> usize {
    let mut p = pos;
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Find the nearest valid char boundary at or after `pos`.
fn safe_char_end(text: &str, pos: usize) -> usize {
    let mut p = pos;
    while p < text.len() && !text.is_char_boundary(p) {
        p += 1;
    }
    p
}

/// Analyzes surrounding context (~50 chars) around a match to determine
/// if it's in a mathematical context.
fn is_math_context(text: &str, match_start: usize, match_end: usize) -> bool {
    // Extract context window
    let ctx_start = safe_char_start(text, if match_start >= 50 { match_start - 50 } else { 0 });
    let ctx_end = safe_char_end(text, (match_end + 50).min(text.len()));
    let context = &text[ctx_start..ctx_end];
    let context_lower = context.to_lowercase();

    // Positive signals: math keywords nearby
    let math_keywords = [
        "where", "given", "such that", "let ", "define", "denote",
        "equation", "formula", "compute", "calculate",
        "minimize", "maximize", "optimal", "converge",
        "theorem", "lemma", "proof", "corollary",
        "ratio", "element", "matrix", "vector", "scalar",
        "function", "variable", "coefficient", "parameter",
        "derivative", "integral", "gradient", "norm",
        "summation", "product", "limit", "approaches",
        "subject to", "constraint", "objective",
    ];

    for keyword in &math_keywords {
        if context_lower.contains(keyword) {
            return true;
        }
    }

    // Positive signal: Greek letters or math symbols nearby
    if MATH_CHAR_PATTERN.is_match(context) {
        return true;
    }

    // Positive signal: existing <math> tags nearby
    if context.contains("<math>") || context.contains("</math>") {
        return true;
    }

    // Negative signals: statistical/reference context
    let negative_keywords = [
        "significance", "confidence", "p-value", "p value",
        "sample size", "participants", "subjects",
        "figure", "table", "section", "chapter",
        "page", "appendix",
    ];

    for keyword in &negative_keywords {
        if context_lower.contains(keyword) {
            return false;
        }
    }

    // Default: not enough context to decide, allow the match
    false
}

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
        let has_math_chars = MATH_CHAR_PATTERN.is_match(matched);
        let has_math_structure = MATH_STRUCTURE_PATTERN.is_match(matched);

        if has_math_chars || has_math_structure {
            // Check for false positives using surrounding context
            let ctx_start = if start >= 20 { start - 20 } else { 0 };
            let ctx_end = (end + 20).min(text.len());
            // Ensure we don't split multi-byte characters
            let ctx_start = safe_char_start(text, ctx_start);
            let ctx_end = safe_char_end(text, ctx_end);
            let context = &text[ctx_start..ctx_end];

            if MATH_FALSE_POSITIVE_PATTERN.is_match(context) {
                // False positive detected, keep as-is
                result.push_str(&text[last_end..end]);
            } else if !has_math_chars && !is_math_context(text, start, end) {
                // Structure-only match without math context - skip
                result.push_str(&text[last_end..end]);
            } else {
                // Add text before this match
                result.push_str(&text[last_end..start]);
                // Wrap match in math tags
                result.push_str("<math>");
                result.push_str(matched);
                result.push_str("</math>");
            }
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

/// Normalize text for fuzzy matching.
///
/// Lowercases text, collapses whitespace sequences to single spaces,
/// removes ASCII punctuation (keeps Unicode letters like Greek), and trims.
///
/// # Arguments
///
/// * `text` - The text to normalize.
///
/// # Returns
///
/// A normalized string suitable for trigram-based comparison.
pub fn normalize_for_matching(text: &str) -> String {
    let lower = text.to_lowercase();
    let filtered: String =
        lower.chars().map(|c| if c.is_ascii_punctuation() { ' ' } else { c }).collect();
    // Collapse whitespace sequences to single space
    let collapsed: String = filtered.split_whitespace().collect::<Vec<&str>>().join(" ");
    collapsed.trim().to_string()
}

/// Build a set of character-level trigrams from a string.
fn trigrams(s: &str) -> HashSet<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut set = HashSet::new();
    if chars.len() < 3 {
        // For very short strings, use the whole string as a single "trigram"
        if !chars.is_empty() {
            set.insert(s.to_string());
        }
        return set;
    }
    for i in 0..chars.len() - 2 {
        let tri: String = chars[i..i + 3].iter().collect();
        set.insert(tri);
    }
    set
}

/// Find the best alignment of `needle` within `haystack` using trigram Jaccard similarity.
///
/// Slides a window of length +/-30% of needle across haystack and finds the
/// position with highest Jaccard similarity (intersection/union of trigram sets).
///
/// # Arguments
///
/// * `needle` - The text to search for (should be normalized).
/// * `haystack` - The text to search within (should be normalized).
///
/// # Returns
///
/// `Some((start, end))` if best similarity >= 0.4, `None` otherwise.
pub fn find_best_alignment(needle: &str, haystack: &str) -> Option<(usize, usize)> {
    if needle.is_empty() || haystack.is_empty() {
        return None;
    }

    let needle_chars: Vec<char> = needle.chars().collect();
    let haystack_chars: Vec<char> = haystack.chars().collect();

    let needle_len = needle_chars.len();
    let haystack_len = haystack_chars.len();

    if needle_len == 0 || haystack_len == 0 {
        return None;
    }

    let needle_trigrams = trigrams(needle);
    if needle_trigrams.is_empty() {
        return None;
    }

    // Window sizes: needle_len +/- 30%
    let min_window = (needle_len as f64 * 0.7).floor() as usize;
    let max_window = (needle_len as f64 * 1.3).ceil() as usize;
    let min_window = min_window.max(1);
    let max_window = max_window.min(haystack_len);

    let mut best_score: f64 = 0.0;
    let mut best_start: usize = 0;
    let mut best_end: usize = 0;

    for window_size in min_window..=max_window {
        if window_size > haystack_len {
            continue;
        }
        for start in 0..=(haystack_len - window_size) {
            let end = start + window_size;
            let window_str: String = haystack_chars[start..end].iter().collect();
            let window_trigrams = trigrams(&window_str);

            if window_trigrams.is_empty() {
                continue;
            }

            // Jaccard similarity
            let intersection = needle_trigrams.intersection(&window_trigrams).count();
            let union = needle_trigrams.union(&window_trigrams).count();

            if union == 0 {
                continue;
            }

            let score = intersection as f64 / union as f64;
            if score > best_score {
                best_score = score;
                best_start = start;
                best_end = end;
            }
        }
    }

    if best_score >= 0.4 {
        Some((best_start, best_end))
    } else {
        None
    }
}

/// Align LLM-converted text to individual blocks using trigram fuzzy matching.
///
/// Splits LLM text into paragraphs (by double newlines), then for each block,
/// finds the best-matching paragraph. If a match is found (similarity >= 0.4),
/// the original (non-normalized) paragraph is used as the math text. Otherwise,
/// falls back to `mark_math_heuristic`.
///
/// # Arguments
///
/// * `llm_converted_text` - The LLM output with `<math>` tags already applied.
/// * `blocks` - The blocks to align against.
///
/// # Returns
///
/// A `HashMap<usize, String>` mapping block index to math-marked text.
pub fn align_llm_text_to_blocks(
    llm_converted_text: &str,
    blocks: &[crate::models::Block],
) -> HashMap<usize, String> {
    let mut result = HashMap::new();

    // Split LLM text into paragraphs by double newlines
    let paragraphs: Vec<&str> =
        llm_converted_text.split("\n\n").map(|p| p.trim()).filter(|p| !p.is_empty()).collect();

    // Pre-normalize all paragraphs
    let normalized_paragraphs: Vec<String> =
        paragraphs.iter().map(|p| normalize_for_matching(p)).collect();

    for (block_idx, block) in blocks.iter().enumerate() {
        let block_text = block.get_text();
        if block_text.trim().is_empty() {
            continue;
        }

        let normalized_block = normalize_for_matching(&block_text);
        if normalized_block.is_empty() {
            continue;
        }

        let mut best_para_idx: Option<usize> = None;
        let mut best_score: f64 = 0.0;

        for (para_idx, norm_para) in normalized_paragraphs.iter().enumerate() {
            if norm_para.is_empty() {
                continue;
            }

            // Try to find alignment of block text within this paragraph
            let block_trigrams = trigrams(&normalized_block);
            let para_trigrams = trigrams(norm_para);

            if block_trigrams.is_empty() || para_trigrams.is_empty() {
                continue;
            }

            // Compute Jaccard similarity between block and paragraph trigrams
            // Also try find_best_alignment for substring matching
            let intersection = block_trigrams.intersection(&para_trigrams).count();
            let union = block_trigrams.union(&para_trigrams).count();

            let jaccard = if union > 0 {
                intersection as f64 / union as f64
            } else {
                0.0
            };

            // Also check substring alignment for cases where block text is
            // a substring of the paragraph
            let alignment_score = if let Some(_) = find_best_alignment(&normalized_block, norm_para)
            {
                // If find_best_alignment succeeds, it means similarity >= 0.4
                // Use the Jaccard score but boost it slightly
                jaccard.max(0.4)
            } else {
                jaccard
            };

            if alignment_score > best_score {
                best_score = alignment_score;
                best_para_idx = Some(para_idx);
            }
        }

        if best_score >= 0.4 {
            if let Some(para_idx) = best_para_idx {
                // Use the original (non-normalized) paragraph text which has math tags
                result.insert(block_idx, paragraphs[para_idx].to_string());
            }
        } else {
            // Fallback to heuristic
            let heuristic = mark_math_heuristic(&block_text);
            if heuristic != block_text {
                result.insert(block_idx, heuristic);
            }
        }
    }

    result
}


/// Unicode math symbols to LaTeX conversion table.
///
/// Maps common Unicode math characters to their LaTeX command equivalents.
/// Used by `unicode_math_to_latex` to normalize math tag contents.
const UNICODE_TO_LATEX: &[(&str, &str)] = &[
    // Lowercase Greek
    ("α", "\\alpha"),
    ("β", "\\beta"),
    ("γ", "\\gamma"),
    ("δ", "\\delta"),
    ("ε", "\\epsilon"),
    ("ζ", "\\zeta"),
    ("η", "\\eta"),
    ("θ", "\\theta"),
    ("ι", "\\iota"),
    ("κ", "\\kappa"),
    ("λ", "\\lambda"),
    ("μ", "\\mu"),
    ("ν", "\\nu"),
    ("ξ", "\\xi"),
    ("π", "\\pi"),
    ("ρ", "\\rho"),
    ("σ", "\\sigma"),
    ("τ", "\\tau"),
    ("υ", "\\upsilon"),
    ("φ", "\\phi"),
    ("χ", "\\chi"),
    ("ψ", "\\psi"),
    ("ω", "\\omega"),
    // Uppercase Greek (only non-Latin-looking ones)
    ("Γ", "\\Gamma"),
    ("Δ", "\\Delta"),
    ("Θ", "\\Theta"),
    ("Λ", "\\Lambda"),
    ("Ξ", "\\Xi"),
    ("Π", "\\Pi"),
    ("Σ", "\\Sigma"),
    ("Φ", "\\Phi"),
    ("Ψ", "\\Psi"),
    ("Ω", "\\Omega"),
    // Relation operators
    ("≤", "\\leq"),
    ("≥", "\\geq"),
    ("≠", "\\neq"),
    ("≈", "\\approx"),
    ("≡", "\\equiv"),
    ("∝", "\\propto"),
    ("≪", "\\ll"),
    ("≫", "\\gg"),
    ("≺", "\\prec"),
    ("≻", "\\succ"),
    // Set operators
    ("∈", "\\in"),
    ("∉", "\\notin"),
    ("⊂", "\\subset"),
    ("⊃", "\\supset"),
    ("⊆", "\\subseteq"),
    ("⊇", "\\supseteq"),
    ("∪", "\\cup"),
    ("∩", "\\cap"),
    ("∅", "\\emptyset"),
    // Large operators
    ("∑", "\\sum"),
    ("∫", "\\int"),
    ("∏", "\\prod"),
    ("∮", "\\oint"),
    // Misc symbols
    ("√", "\\sqrt"),
    ("∞", "\\infty"),
    ("∂", "\\partial"),
    ("∇", "\\nabla"),
    // Arithmetic
    ("±", "\\pm"),
    ("∓", "\\mp"),
    ("×", "\\times"),
    ("÷", "\\div"),
    ("·", "\\cdot"),
    // Arrows
    ("→", "\\to"),
    ("←", "\\leftarrow"),
    ("↔", "\\leftrightarrow"),
    ("⇒", "\\Rightarrow"),
    ("⇐", "\\Leftarrow"),
    ("⇔", "\\Leftrightarrow"),
    ("↑", "\\uparrow"),
    ("↓", "\\downarrow"),
    // Logic
    ("∀", "\\forall"),
    ("∃", "\\exists"),
    ("¬", "\\neg"),
    ("∧", "\\land"),
    ("∨", "\\lor"),
    // Dots
    ("⋯", "\\cdots"),
    ("⋮", "\\vdots"),
    ("⋱", "\\ddots"),
    // Superscript digits to ^{n}
    ("⁰", "^{0}"),
    ("¹", "^{1}"),
    ("²", "^{2}"),
    ("³", "^{3}"),
    ("⁴", "^{4}"),
    ("⁵", "^{5}"),
    ("⁶", "^{6}"),
    ("⁷", "^{7}"),
    ("⁸", "^{8}"),
    ("⁹", "^{9}"),
    ("ⁿ", "^{n}"),
    // Subscript digits to _{n}
    ("₀", "_{0}"),
    ("₁", "_{1}"),
    ("₂", "_{2}"),
    ("₃", "_{3}"),
    ("₄", "_{4}"),
    ("₅", "_{5}"),
    ("₆", "_{6}"),
    ("₇", "_{7}"),
    ("₈", "_{8}"),
    ("₉", "_{9}"),
    ("ₙ", "_{n}"),
];

/// Regex pattern for matching `<math>` and `<math display="block">` segments.
static MATH_TAG_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<math(?:\s+display="block")?>[\s\S]*?</math>"#).unwrap()
});

/// Convert Unicode math symbols inside `<math>...</math>` tags to LaTeX equivalents.
///
/// This function finds all `<math>...</math>` and `<math display="block">...</math>`
/// segments in the text, and for the content inside those tags only, replaces Unicode
/// math symbols with their LaTeX command equivalents. Text outside of math tags is
/// left unchanged.
///
/// If the content already contains LaTeX commands (backslash sequences), those are
/// preserved without double-replacement.
///
/// # Arguments
///
/// * `text` - The text potentially containing `<math>` tags with Unicode math symbols.
///
/// # Returns
///
/// The text with Unicode math symbols inside math tags converted to LaTeX commands.
pub fn unicode_math_to_latex(text: &str) -> String {
    if text.is_empty() || !text.contains("<math") {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len() * 2);
    let mut last_end = 0;

    for mat in MATH_TAG_PATTERN.find_iter(text) {
        let full_match = mat.as_str();

        // Append text before this match (outside math tags) unchanged
        result.push_str(&text[last_end..mat.start()]);

        // Find where the opening tag ends and closing tag starts
        let inner_start = full_match.find('>').unwrap() + 1;
        let inner_end = full_match.rfind("</math>").unwrap();
        let opening_tag = &full_match[..inner_start];
        let inner_content = &full_match[inner_start..inner_end];
        let closing_tag = "</math>";

        // Convert Unicode to LaTeX in the inner content
        let mut converted = inner_content.to_string();
        for &(unicode, latex) in UNICODE_TO_LATEX {
            if converted.contains(unicode) {
                converted = converted.replace(unicode, latex);
            }
        }

        result.push_str(opening_tag);
        result.push_str(&converted);
        result.push_str(closing_tag);

        last_end = mat.end();
    }

    // Append remaining text after the last match
    result.push_str(&text[last_end..]);
    result
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

    #[test]
    fn test_get_model_id_default() {
        // Clear the environment variable to test default
        std::env::remove_var("OPENAI_API_MODEL");
        let model = get_model_id();
        assert_eq!(model, "gpt-5.2");
    }

    #[test]
    fn test_get_model_id_custom() {
        // Set custom model
        std::env::set_var("OPENAI_API_MODEL", "gpt-4o");
        let model = get_model_id();
        assert_eq!(model, "gpt-4o");
        // Clean up
        std::env::remove_var("OPENAI_API_MODEL");
    }

    #[test]
    fn test_parse_references_json_clean() {
        let json = r#"[
            {
                "authors": ["John Smith", "Jane Doe"],
                "title": "A Great Paper",
                "year": 2023,
                "venue": "NeurIPS",
                "doi": null,
                "url": null,
                "arxiv_id": "2308.10379",
                "volume": null,
                "pages": null
            }
        ]"#;

        let refs = parse_references_json(json).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].title, Some("A Great Paper".to_string()));
        assert_eq!(refs[0].year, Some(2023));
        assert_eq!(
            refs[0].authors,
            Some(vec!["John Smith".to_string(), "Jane Doe".to_string()])
        );
        assert_eq!(refs[0].arxiv_id, Some("2308.10379".to_string()));
        assert_eq!(refs[0].doi, None);
    }

    #[test]
    fn test_parse_references_json_with_surrounding_text() {
        let json = r#"Here is the parsed JSON:

[
    {
        "authors": ["Alice"],
        "title": "Test",
        "year": 2024,
        "venue": "ICML",
        "doi": "10.1234/test",
        "url": null,
        "arxiv_id": null,
        "volume": "42",
        "pages": "1-10"
    }
]

Hope this helps!"#;

        let refs = parse_references_json(json).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].title, Some("Test".to_string()));
        assert_eq!(refs[0].doi, Some("10.1234/test".to_string()));
        assert_eq!(refs[0].volume, Some("42".to_string()));
        assert_eq!(refs[0].pages, Some("1-10".to_string()));
    }

    #[test]
    fn test_parse_references_json_invalid() {
        let json = "This is not valid JSON";
        let refs = parse_references_json(json).unwrap();
        assert!(refs.is_empty());
    }

    #[test]
    fn test_parse_references_json_multiple() {
        let json = r#"[
            {"authors": ["A"], "title": "First", "year": 2020, "venue": "V1", "doi": null, "url": null, "arxiv_id": null, "volume": null, "pages": null},
            {"authors": ["B"], "title": "Second", "year": 2021, "venue": "V2", "doi": null, "url": null, "arxiv_id": null, "volume": null, "pages": null},
            {"authors": ["C"], "title": "Third", "year": 2022, "venue": "V3", "doi": null, "url": null, "arxiv_id": null, "volume": null, "pages": null}
        ]"#;

        let refs = parse_references_json(json).unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].title, Some("First".to_string()));
        assert_eq!(refs[1].title, Some("Second".to_string()));
        assert_eq!(refs[2].title, Some("Third".to_string()));
    }

    #[test]
    fn test_normalize_for_matching() {
        assert_eq!(normalize_for_matching("  Hello,  World! "), "hello world");
        assert_eq!(
            normalize_for_matching("\u{03b1} + \u{03b2} = \u{03b3}"),
            "\u{03b1} \u{03b2} \u{03b3}"
        ); // punctuation removed but unicode kept
    }

    #[test]
    fn test_find_best_alignment_exact() {
        let needle = "the quick brown fox";
        let haystack = "once upon a time the quick brown fox jumped over the lazy dog";
        let result = find_best_alignment(needle, haystack);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_best_alignment_fuzzy() {
        let needle = "the quikc brown fox"; // typo
        let haystack = "once upon a time the quick brown fox jumped over the lazy dog";
        let result = find_best_alignment(needle, haystack);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_best_alignment_no_match() {
        let needle = "completely unrelated text about cooking";
        let haystack = "mathematical equations and formulas for physics";
        let result = find_best_alignment(needle, haystack);
        assert!(result.is_none());
    }

    #[test]
    fn test_align_llm_text_to_blocks() {
        use crate::models::Block;

        // Create test blocks
        let mut block1 = Block::new(0.0, 0.0, 100.0, 20.0);
        block1.add_line(0.0, 0.0, 100.0, 10.0);
        block1.lines[0].add_word("The".to_string(), 0.0, 0.0, 10.0, 10.0);
        block1.lines[0].add_word("equation".to_string(), 12.0, 0.0, 30.0, 10.0);

        let mut block2 = Block::new(0.0, 30.0, 100.0, 20.0);
        block2.add_line(0.0, 30.0, 100.0, 10.0);
        block2.lines[0].add_word("Results".to_string(), 0.0, 30.0, 20.0, 10.0);
        block2.lines[0].add_word("show".to_string(), 22.0, 30.0, 15.0, 10.0);

        let blocks = vec![block1, block2];

        let llm_text = "The equation <math>f(x) = ax^2</math>\n\nResults show improvement";
        let aligned = align_llm_text_to_blocks(llm_text, &blocks);

        // Block 0 should get LLM text with math tag
        assert!(aligned.get(&0).map_or(false, |t| t.contains("<math>")));
        // Block 1 should get text (either aligned or heuristic fallback)
        assert!(aligned.contains_key(&1));
    }

    // Phase 2 tests - False Positive Reduction
    #[test]
    fn test_no_false_positive_dates() {
        let text = "published in 2019/2020 and later in 2021/2022";
        let marked = mark_math_heuristic(text);
        assert!(
            !marked.contains("<math>"),
            "Dates should not be marked as math: {}",
            marked
        );
    }

    #[test]
    fn test_no_false_positive_statistics() {
        let text = "n = 50 participants were recruited";
        let marked = mark_math_heuristic(text);
        assert!(
            !marked.contains("<math>"),
            "Statistical reporting should not be marked as math: {}",
            marked
        );
    }

    #[test]
    fn test_no_false_positive_parenthetical() {
        let text = "as shown in (a) and discussed in (b)";
        let marked = mark_math_heuristic(text);
        assert!(
            !marked.contains("<math>"),
            "Parenthetical references should not be marked as math: {}",
            marked
        );
    }

    #[test]
    fn test_real_math_still_detected() {
        let text = "where α ≤ β for all cases";
        let marked = mark_math_heuristic(text);
        assert!(
            marked.contains("<math>"),
            "Real math should still be detected: {}",
            marked
        );
        assert!(marked.contains("α"));
        assert!(marked.contains("β"));
    }

    // Phase 3 tests - False Negative Fixes
    #[test]
    fn test_multichar_functions() {
        let text = "where sin(x) + cos(θ) = 1";
        let marked = mark_math_heuristic(text);
        assert!(
            marked.contains("<math>"),
            "Multi-char math functions should be detected: {}",
            marked
        );
    }

    #[test]
    fn test_ascii_exponents() {
        let text = "compute x^2 + y^2";
        let marked = mark_math_heuristic(text);
        assert!(
            marked.contains("<math>"),
            "ASCII exponents should be detected: {}",
            marked
        );
    }

    #[test]
    fn test_ascii_subscripts() {
        let text = "element x_i for i in S";
        let marked = mark_math_heuristic(text);
        assert!(
            marked.contains("<math>"),
            "ASCII subscripts should be detected: {}",
            marked
        );
    }

    #[test]
    fn test_letter_fractions() {
        let text = "the ratio a/b approaches zero";
        let marked = mark_math_heuristic(text);
        assert!(
            marked.contains("<math>"),
            "Letter fractions should be detected: {}",
            marked
        );
    }

    #[test]
    fn test_norm_notation() {
        let text = "minimize ||w|| subject to constraints";
        let marked = mark_math_heuristic(text);
        assert!(
            marked.contains("<math>"),
            "Norm notation should be detected: {}",
            marked
        );
    }

    // Phase 4 tests - Unicode to LaTeX conversion
    #[test]
    fn test_unicode_to_latex_greek() {
        let input = "<math>\u{03b1} + \u{03b2}</math>";
        let result = unicode_math_to_latex(input);
        assert_eq!(result, "<math>\\alpha + \\beta</math>");
    }

    #[test]
    fn test_unicode_to_latex_operators() {
        let input = "<math>x \u{2264} y</math>";
        let result = unicode_math_to_latex(input);
        assert_eq!(result, "<math>x \\leq y</math>");
    }

    #[test]
    fn test_unicode_to_latex_preserves_outside() {
        let input = "The variable \u{03b1} is defined as <math>\u{03b1} + \u{03b2}</math> in the paper.";
        let result = unicode_math_to_latex(input);
        assert_eq!(
            result,
            "The variable \u{03b1} is defined as <math>\\alpha + \\beta</math> in the paper."
        );
    }

    #[test]
    fn test_unicode_to_latex_already_latex() {
        let input = "<math>\\alpha + \\beta</math>";
        let result = unicode_math_to_latex(input);
        assert_eq!(result, "<math>\\alpha + \\beta</math>");
    }

    #[test]
    fn test_unicode_to_latex_mixed() {
        let input = "Text <math>\u{03b1} \u{2264} \u{03b2}</math> more text <math>x \u{2208} S</math> end.";
        let result = unicode_math_to_latex(input);
        assert_eq!(
            result,
            "Text <math>\\alpha \\leq \\beta</math> more text <math>x \\in S</math> end."
        );
    }

    #[test]
    fn test_unicode_to_latex_display_block() {
        let input = "<math display=\"block\">\u{2211} \u{03b1}</math>";
        let result = unicode_math_to_latex(input);
        assert!(result.contains("\\sum"));
        assert!(result.contains("\\alpha"));
    }

    #[test]
    fn test_unicode_to_latex_subscript_superscript() {
        let input = "<math>x\u{00b2} + y\u{2083}</math>";
        let result = unicode_math_to_latex(input);
        assert_eq!(result, "<math>x^{2} + y_{3}</math>");
    }

    // Phase 5 tests - Context-based Math Detection
    #[test]
    fn test_context_equation_preamble() {
        let text = "where f(x) = ax + b represents the model";
        let marked = mark_math_heuristic(text);
        assert!(marked.contains("<math>"), "Math context with 'where' keyword should tag: {}", marked);
    }

    #[test]
    fn test_context_statistical_report() {
        let text = "with p < 0.05 significance level was observed";
        let marked = mark_math_heuristic(text);
        assert!(!marked.contains("<math>"), "Statistical context should not tag: {}", marked);
    }

    #[test]
    fn test_context_greek_nearby() {
        let text = "the value f(x) where α = 3";
        let marked = mark_math_heuristic(text);
        assert!(marked.contains("<math>"), "Greek letters nearby should confirm math context: {}", marked);
    }

    /// Comprehensive regression suite for math heuristic detection.
    /// Tests all known patterns for false positives and false negatives.
    #[test]
    fn test_heuristic_regression_suite() {
        // === FALSE POSITIVES: These should NOT be tagged ===
        let false_positives = vec![
            // Dates
            ("published in 2019/2020", "date with slash"),
            ("from 01/15/2023 onwards", "date format"),
            // Statistical reports
            ("n = 50 participants in the study", "statistical n"),
            // Parenthetical references
            ("as shown in (a) above", "parenthetical ref"),
            ("see method(s) for details", "parenthetical plural"),
            // Section references
            ("described in Section 3.1 below", "section ref"),
            ("see Figure 2 for details", "figure ref"),
            ("Table 1 shows the results", "table ref"),
            // Plain text
            ("This is a normal sentence.", "plain text"),
            ("The product was released.", "word 'product'"),
        ];

        for (text, label) in &false_positives {
            let marked = mark_math_heuristic(text);
            assert!(
                !marked.contains("<math>"),
                "FALSE POSITIVE [{}]: '{}' was incorrectly tagged as math: {}",
                label, text, marked
            );
        }

        // === TRUE POSITIVES: These SHOULD be tagged ===
        let true_positives = vec![
            // Greek letters
            ("The learning rate α converges", "greek alpha"),
            ("where β ≥ 0", "greek with operator"),
            // Operators
            ("we have x ∈ S", "set membership"),
            ("when a ≤ b", "inequality with Greek-range operator"),
            // Equations with context
            ("where f(x) = ax + b", "equation with 'where'"),
            // Subscripts/superscripts
            ("compute x^2 + y^2", "ascii exponents"),
            ("element x_i in the set", "ascii subscripts"),
            // Math functions
            ("where sin(x) = 0", "trig function with context"),
            ("compute log(n) time", "log function with context"),
            // Norm
            ("minimize ||w|| subject to constraints", "norm notation"),
            // Unicode math symbols
            ("the sum ∑ of all values", "summation symbol"),
            ("the integral ∫ over space", "integral symbol"),
        ];

        for (text, label) in &true_positives {
            let marked = mark_math_heuristic(text);
            assert!(
                marked.contains("<math>"),
                "FALSE NEGATIVE [{}]: '{}' was NOT tagged as math: {}",
                label, text, marked
            );
        }
    }
}
