use anyhow::Result;
use openai_tools::chat::request::ChatCompletion;
use openai_tools::common::message::{Content, Message};
use openai_tools::common::role::Role;

use crate::config::PageNumber;

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
