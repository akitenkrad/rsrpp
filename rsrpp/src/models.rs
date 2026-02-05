use crate::config::PageNumber;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Block type classification for text blocks in a PDF document.
///
/// # Variants
///
/// * `Body` - Normal body text (default)
/// * `Caption` - Figure/table captions (e.g., "Figure 1: ...")
/// * `Header` - Section headers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum BlockType {
    #[default]
    Body,
    Caption,
    Header,
}

/// Rich text representation with original and math-marked versions.
///
/// # Fields
///
/// * `original` - The original text content
/// * `math_marked` - Optional text with math expressions marked using `<math>...</math>` tags
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichText {
    pub original: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math_marked: Option<String>,
}

/// List of suffixes that should be hyphenated when preceded by a word.
/// Example: "databased" -> "data-based", "eventdriven" -> "event-driven"
const HYPHENATED_SUFFIXES: &[&str] = &[
    "based",
    "driven",
    "oriented",
    "aware",
    "agnostic",
    "independent",
    "dependent",
    "first",
    "native",
    "centric",
    "intensive",
    "bound",
    "safe",
    "free",
    "proof",
    "efficient",
    "optimized",
    "enabled",
    "powered",
    "ready",
    "capable",
    "compatible",
    "compliant",
    "level",
    "scale",
    "wide",
    "specific",
    "friendly",
    "facing",
    "like",
    "style",
];

/// Pre-compiled regexes for suffix hyphenation.
/// Each suffix has its own regex, sorted by length (longest first) to ensure
/// proper matching (e.g., "independent" matches before "dependent").
/// Uses \b at start to prevent re-matching already-hyphenated words.
static SUFFIX_REGEXES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let mut suffixes: Vec<&str> = HYPHENATED_SUFFIXES.to_vec();
    suffixes.sort_by(|a, b| b.len().cmp(&a.len()));
    suffixes
        .into_iter()
        .map(|suffix| {
            let pattern = format!(r"\b[A-Za-z]+{}\b", suffix);
            (Regex::new(&pattern).unwrap(), suffix)
        })
        .collect()
});

/// Fixes compound words that should have hyphens before specific suffixes.
/// Example: "databased" -> "data-based", "userdriven" -> "user-driven"
pub fn fix_suffix_hyphens(text: &str) -> String {
    let mut result = text.to_string();
    for (regex, suffix) in SUFFIX_REGEXES.iter() {
        let current = result.clone();
        result = regex
            .replace_all(&current, |caps: &regex::Captures| {
                let m = caps.get(0).unwrap();
                let matched = m.as_str();
                let start_pos = m.start();

                // Skip if preceded by a hyphen (already hyphenated compound)
                if start_pos > 0 {
                    let prev_in_text = current.as_bytes()[start_pos - 1] as char;
                    if prev_in_text == '-' {
                        return matched.to_string();
                    }
                }

                let suffix_pos = matched.len() - suffix.len();
                if suffix_pos > 0 {
                    let prev_char = matched.as_bytes()[suffix_pos - 1] as char;
                    if prev_char != '-' && prev_char != ' ' {
                        let (head, _) = matched.split_at(suffix_pos);
                        return format!("{}-{}", head, suffix);
                    }
                }
                matched.to_string()
            })
            .to_string();
    }
    result
}

/// The `Word` struct represents a word in a PDF document.
///
/// # Fields
///
/// * `text` - The text content of the word.
/// * `x` - The x-coordinate of the top-left corner of the word.
/// * `y` - The y-coordinate of the top-left corner of the word.
/// * `width` - The width of the word.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Word {
    pub fn font_size(&self) -> f32 {
        return self.height;
    }
}

/// The `Line` struct represents a line of text in a PDF document.
///
/// # Fields
///
/// * `words` - A vector of `Word` structs that make up the line.
/// * `x` - The x-coordinate of the top-left corner of the line.
/// * `y` - The y-coordinate of the top-left corner of the line.
/// * `width` - The width of the line.
/// * `height` - The height of the line.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub words: Vec<Word>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Line {
    /// Creates a new `Line` instance.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the top-left corner of the line.
    /// * `y` - The y-coordinate of the top-left corner of the line.
    /// * `width` - The width of the line.
    /// * `height` - The height of the line.
    ///
    /// # Returns
    ///
    /// A new `Line` instance with the specified coordinates and dimensions.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Line {
        Line {
            words: Vec::new(),
            x: x,
            y: y,
            width: width,
            height: height,
        }
    }
    /// Adds a new `Word` to the `Line`.
    ///
    /// # Arguments
    ///
    /// * `text` - The text content of the word.
    /// * `x` - The x-coordinate of the top-left corner of the word.
    /// * `y` - The y-coordinate of the top-left corner of the word.
    /// * `width` - The width of the word.
    /// * `height` - The height of the word.
    pub fn add_word(&mut self, text: String, x: f32, y: f32, width: f32, height: f32) {
        self.words.push(Word {
            text: text.trim().to_string(),
            x: x,
            y: y,
            width: width,
            height: height,
        });
    }
    /// Returns the concatenated text of all `Word` instances in the `Line`.
    ///
    /// # Returns
    ///
    /// A `String` containing the text of all words in the line, separated by spaces.
    pub fn get_text(&self) -> String {
        let mut words = Vec::new();
        for word in &self.words {
            words.push(word.text.clone());
        }
        return words.join(" ");
    }
}

/// The `Block` struct represents a block of text in a PDF document.
///
/// # Fields
///
/// * `lines` - A vector of `Line` structs that make up the block.
/// * `x` - The x-coordinate of the top-left corner of the block.
/// * `y` - The y-coordinate of the top-left corner of the block.
/// * `width` - The width of the block.
/// * `height` - The height of the block.
/// * `section` - The section of the document to which the block belongs.
/// * `block_type` - The classification of the block (Body, Caption, or Header).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub lines: Vec<Line>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub section: String,
    pub block_type: BlockType,
}

impl Block {
    /// Creates a new `Block` instance.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the top-left corner of the block.
    /// * `y` - The y-coordinate of the top-left corner of the block.
    /// * `width` - The width of the block.
    /// * `height` - The height of the block.
    ///
    /// # Returns
    ///
    /// A new `Block` instance with the specified coordinates and dimensions.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Block {
        Block {
            lines: Vec::new(),
            x: x,
            y: y,
            width: width,
            height: height,
            section: String::new(),
            block_type: BlockType::default(),
        }
    }
    /// Adds a new `Line` to the `Block`.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the top-left corner of the line.
    /// * `y` - The y-coordinate of the top-left corner of the line.
    /// * `width` - The width of the line.
    /// * `height` - The height of the line.
    pub fn add_line(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.lines.push(Line::new(x, y, width, height));
    }

    /// Returns the concatenated text of all `Line` instances in the `Block`.
    ///
    /// # Returns
    ///
    /// A `String` containing the text of all lines in the block, with hyphenated line endings removed.
    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text = text.trim().to_string();
            if text.ends_with("-") {
                // 意味を壊すよりも、表記上の崩壊に逃げる
                text = text.trim().trim_end_matches("-").to_string();
                // ハイフン終わりの時はスペースいらない
            } else {
                text.push_str(" ");
            }
            //      text = text.trim().trim_end_matches("-").to_string();
            //     text.push_str(" ");
            text.push_str(&line.get_text());
        }

        text = fix_suffix_hyphens(&text);
        return text.trim().to_string();
    }
}

/// The `Page` struct represents a page in a PDF document.
///
/// # Fields
///
/// * `blocks` - A vector of `Block` structs that make up the page.
/// * `width` - The width of the page.
/// * `height` - The height of the page.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub blocks: Vec<Block>,
    pub width: f32,
    pub height: f32,
    pub tables: Vec<Coordinate>,
    pub page_number: PageNumber,
    pub number_of_columns: i8,
}

impl Page {
    /// Creates a new `Page` instance.
    ///
    /// # Arguments
    ///
    /// * `width` - The width of the page.
    /// * `height` - The height of the page.
    /// * `page_number` - The page number.
    ///
    /// # Returns
    ///
    /// A new `Page` instance with the specified dimensions and page number.
    pub fn new(width: f32, height: f32, page_number: PageNumber) -> Page {
        Page {
            blocks: Vec::new(),
            width: width,
            height: height,
            tables: Vec::new(),
            page_number,
            number_of_columns: 1,
        }
    }

    /// Adds a new `Block` to the `Page`.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the top-left corner of the block.
    /// * `y` - The y-coordinate of the top-left corner of the block.
    /// * `width` - The width of the block.
    /// * `height` - The height of the block.
    pub fn add_block(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.blocks.push(Block::new(x, y, width, height));
    }

    /// Returns the concatenated text of all `Block` instances in the `Page`.
    ///
    /// # Returns
    ///
    /// A `String` containing the text of all blocks in the page, separated by double newlines.
    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for block in &self.blocks {
            text.push_str(&block.get_text());
            text.push_str("\n\n");
        }
        return text;
    }

    /// Returns the y-coordinate of the topmost line in the page.
    ///
    /// # Returns
    ///
    /// `Some(f32)` representing the y-coordinate of the topmost line, or `None` if the page has no lines.
    pub fn top(&self) -> Option<f32> {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.y);
            }
        }
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values.first().copied()
    }

    /// Returns the y-coordinate of the bottommost line in the page.
    ///
    /// # Returns
    ///
    /// `Some(f32)` representing the y-coordinate of the bottommost line, or `None` if the page has no lines.
    pub fn bottom(&self) -> Option<f32> {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.y + line.height);
            }
        }
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        values.first().copied()
    }

    /// Returns the x-coordinate of the leftmost line in the page.
    ///
    /// # Returns
    ///
    /// `Some(f32)` representing the x-coordinate of the leftmost line, or `None` if the page has no lines.
    pub fn left(&self) -> Option<f32> {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.x);
            }
        }
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values.first().copied()
    }

    /// Returns the x-coordinate of the rightmost line in the page.
    ///
    /// # Returns
    ///
    /// `Some(f32)` representing the x-coordinate of the rightmost line, or `None` if the page has no lines.
    pub fn right(&self) -> Option<f32> {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.x + line.width);
            }
        }
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        values.first().copied()
    }
}

/// The `Point` struct represents a point in 2D space.
///
/// # Fields
///
/// * `x` - The x-coordinate of the point.
/// * `y` - The y-coordinate of the point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Point {
        Point { x: x, y: y }
    }
}

/// The `Coordinate` struct represents the coordinates of a rectangular area in 2D space.
///
/// # Fields
///
/// * `top_left` - The top-left corner of the rectangle.
/// * `top_right` - The top-right corner of the rectangle.
/// * `bottom_left` - The bottom-left corner of the rectangle.
/// * `bottom_right` - The bottom-right corner of the rectangle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coordinate {
    pub top_left: Point,
    pub top_right: Point,
    pub bottom_left: Point,
    pub bottom_right: Point,
}

impl Coordinate {
    /// Creates a `Coordinate` instance from the given rectangle coordinates.
    ///
    /// # Arguments
    ///
    /// * `x1` - The x-coordinate of the top-left corner.
    /// * `y1` - The y-coordinate of the top-left corner.
    /// * `x2` - The x-coordinate of the bottom-right corner.
    /// * `y2` - The y-coordinate of the bottom-right corner.
    ///
    /// # Returns
    ///
    /// A `Coordinate` instance representing the rectangle.
    pub fn from_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> Coordinate {
        Coordinate {
            top_left: Point { x: x1, y: y1 },
            top_right: Point { x: x2, y: y1 },
            bottom_left: Point { x: x1, y: y2 },
            bottom_right: Point { x: x2, y: y2 },
        }
    }

    /// Creates a `Coordinate` instance from the given object dimensions.
    ///
    /// # Arguments
    ///
    /// * `x` - The x-coordinate of the top-left corner of the object.
    /// * `y` - The y-coordinate of the top-left corner of the object.
    /// * `width` - The width of the object.
    /// * `height` - The height of the object.
    ///
    /// # Returns
    ///
    /// A `Coordinate` instance representing the object.
    pub fn from_object(x: f32, y: f32, width: f32, height: f32) -> Coordinate {
        Coordinate {
            top_left: Point { x: x, y: y },
            top_right: Point { x: x + width, y: y },
            bottom_left: Point {
                x: x,
                y: y + height,
            },
            bottom_right: Point {
                x: x + width,
                y: y + height,
            },
        }
    }

    /// Returns the width of the rectangle represented by the `Coordinate`.
    ///
    /// # Returns
    ///
    /// A `f32` representing the width of the rectangle.
    pub fn width(&self) -> f32 {
        return self.top_right.x - self.top_left.x;
    }

    /// Returns the height of the rectangle represented by the `Coordinate`.
    ///
    /// # Returns
    ///
    /// A `f32` representing the height of the rectangle.
    pub fn height(&self) -> f32 {
        return self.bottom_left.y - self.top_left.y;
    }

    /// Determines if the rectangle represented by this `Coordinate` intersects with another `Coordinate`.
    ///
    /// # Arguments
    ///
    /// * `other` - Another `Coordinate` to check for intersection.
    ///
    /// # Returns
    ///
    /// A `bool` indicating whether the rectangles intersect.
    pub fn is_intercept(&self, other: &Coordinate) -> bool {
        if self.top_left.x >= other.bottom_right.x || self.bottom_right.x <= other.top_left.x {
            return false;
        }
        if self.top_left.y >= other.bottom_right.y || self.bottom_right.y <= other.top_left.y {
            return false;
        }
        return true;
    }

    /// Returns the area of the rectangle represented by the `Coordinate`.
    ///
    /// # Returns
    ///
    /// A `f32` representing the area of the rectangle.
    pub fn get_area(&self) -> f32 {
        return self.width() * self.height();
    }

    /// Returns the intersection of the rectangle represented by this `Coordinate` with another `Coordinate`.
    ///
    /// # Arguments
    ///
    /// * `other` - Another `Coordinate` to intersect with.
    ///
    /// # Returns
    ///
    /// A `Coordinate` representing the intersected area.
    pub fn intersection(&self, other: &Coordinate) -> Coordinate {
        let x1 = f32::max(self.top_left.x, other.top_left.x);
        let y1 = f32::max(self.top_left.y, other.top_left.y);
        let x2 = f32::min(self.bottom_right.x, other.bottom_right.x);
        let y2 = f32::min(self.bottom_right.y, other.bottom_right.y);
        return Coordinate::from_rect(x1, y1, x2, y2);
    }

    ///
    /// Computes the Intersection over Union (IoU) of the rectangle represented by this `Coordinate` with another `Coordinate`.
    ///
    /// # Arguments
    ///
    /// * `other` - Another `Coordinate` to compute the IoU with.
    ///
    /// # Returns
    ///
    /// A `f32` representing the IoU value, which is the ratio of the intersected area to the union area of the two rectangles.
    pub fn iou(&self, other: &Coordinate) -> f32 {
        let dx = f32::min(self.bottom_right.x, other.bottom_right.x)
            - f32::max(self.top_left.x, other.top_left.x);
        let dy = f32::min(self.bottom_right.y, other.bottom_right.y)
            - f32::max(self.top_left.y, other.top_left.y);

        if dx <= 0.0 || dy <= 0.0 {
            return 0.0;
        } else {
            let area1 = self.width() * self.height();
            let area2 = other.width() * other.height();
            let inter_area = dx * dy;
            return inter_area / (area1 + area2 - inter_area);
        }
    }

    /// Determines if the rectangle represented by this `Coordinate` is contained within another `Coordinate`.
    ///
    /// # Arguments
    ///
    /// * `other` - Another `Coordinate` to check for containment.
    ///
    /// # Returns
    ///
    /// A `bool` indicating whether this rectangle is contained within the other rectangle.
    pub fn is_contained_in(&self, other: &Coordinate) -> bool {
        let iou = self.iou(other);
        let intersection = self.intersection(other).get_area();
        let self_area = self.get_area();
        return iou > 0.0 && intersection / self_area > 0.3;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
    pub coordinates: Coordinate,
}

pub struct TextBlockReference {
    pub text: String,
    pub coordinates: Coordinate,
}

/// A single bibliographic reference extracted from a paper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Raw text of the reference entry (may be null if LLM doesn't return it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_text: Option<String>,
    /// Parsed author list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    /// Title of the referenced work
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Publication year
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Venue (journal, conference, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,
    /// Digital Object Identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    /// URL if present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// arXiv identifier (e.g., "2308.10379")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arxiv_id: Option<String>,
    /// Volume number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
    /// Page range (e.g., "1-15")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<String>,
}

/// Complete paper output with sections and references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperOutput {
    /// All sections of the paper
    pub sections: Vec<Section>,
    /// Extracted references (separate from sections)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<Reference>,
}

impl TextBlock {
    pub fn from_block(block: &Block) -> TextBlock {
        TextBlock {
            text: block.get_text(),
            coordinates: Coordinate::from_object(block.x, block.y, block.width, block.height),
        }
    }
}
/// The `Section` struct represents a section in a PDF document.
///
/// # Fields
///
/// * `index` - The order index of the section.
/// * `title` - The title of the section.
/// * `contents` - The content of the section (original text, captions excluded).
/// * `math_contents` - Optional content with math expressions marked using `<math>...</math>` tags.
/// * `captions` - Figure/table captions belonging to this section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub index: i16,
    pub title: String,
    pub contents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math_contents: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captions: Vec<String>,
}

impl Section {
    /// Creates a vector of `Section` instances from a vector of `Page` instances.
    ///
    /// # Arguments
    ///
    /// * `pages` - A reference to a vector of `Page` instances.
    ///
    /// # Returns
    ///
    /// A vector of `Section` instances, each representing a section in the PDF document.
    /// Captions are separated into a dedicated field and excluded from main contents.
    pub fn from_pages(pages: &Vec<Page>) -> Vec<Section> {
        let mut section_indices: HashMap<String, i16> = HashMap::new();
        let mut section_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut caption_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut last_text = String::new();
        let eos_ptn = regex::Regex::new(r"(\.)(\W)").unwrap();
        let ex_ws_ptn = regex::Regex::new(r"\s+").unwrap();

        for page in pages {
            for block in &page.blocks {
                let keys = section_map.keys().cloned().collect::<Vec<String>>();
                let mut text_block = block.get_text().trim().to_string();

                if text_block.ends_with("-") {
                    last_text.push_str(&text_block.trim_end_matches("-"));
                    continue;
                }

                if !last_text.is_empty() {
                    last_text.push_str(&text_block);
                    text_block = last_text.clone();
                    last_text.clear();
                }

                text_block = eos_ptn.replace_all(&text_block, "$1 $2").to_string();
                text_block = ex_ws_ptn.replace_all(&text_block, " ").to_string();

                // Separate captions from body content
                let is_caption = block.block_type == BlockType::Caption;

                if is_caption {
                    // Add to captions map
                    caption_map
                        .entry(block.section.clone())
                        .or_insert_with(Vec::new)
                        .push(text_block);
                    // Ensure section exists in indices
                    if !section_indices.contains_key(&block.section) {
                        section_indices.insert(block.section.clone(), section_indices.len() as i16);
                    }
                } else {
                    // Add to content map (existing logic)
                    if keys.contains(&block.section) {
                        let content = section_map.get_mut(&block.section).unwrap();
                        content.push(text_block);
                    } else {
                        section_map.insert(block.section.clone(), vec![text_block]);
                        section_indices.insert(block.section.clone(), section_indices.len() as i16);
                    }
                }
            }
        }
        let mut sections = Vec::new();
        for (title, contents) in section_map {
            let captions = caption_map.remove(&title).unwrap_or_default();
            sections.push(Section {
                index: section_indices.get(&title).copied().unwrap_or(0),
                title: title,
                contents: contents,
                math_contents: None, // Will be populated by math markup phase
                captions: captions,
            });
        }
        // Handle sections with only captions (no body content)
        for (title, captions) in caption_map {
            sections.push(Section {
                index: section_indices.get(&title).copied().unwrap_or(0),
                title: title,
                contents: Vec::new(),
                math_contents: None,
                captions: captions,
            });
        }
        sections.sort_by(|a, b| a.index.cmp(&b.index));
        return sections;
    }

    /// Creates a vector of `Section` instances from pages with math markup.
    ///
    /// This is similar to `from_pages` but also populates `math_contents` using
    /// the math_texts map from ParserConfig.
    ///
    /// # Arguments
    ///
    /// * `pages` - A reference to a vector of `Page` instances.
    /// * `math_texts` - A map of (page_number, block_index) to math-marked text.
    ///
    /// # Returns
    ///
    /// A vector of `Section` instances with math_contents populated where applicable.
    pub fn from_pages_with_math(
        pages: &Vec<Page>,
        math_texts: &HashMap<(crate::config::PageNumber, usize), String>,
    ) -> Vec<Section> {
        let mut section_indices: HashMap<String, i16> = HashMap::new();
        let mut section_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut math_section_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut caption_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut last_text = String::new();
        let mut last_math_text = String::new();
        let eos_ptn = regex::Regex::new(r"(\.)(\W)").unwrap();
        let ex_ws_ptn = regex::Regex::new(r"\s+").unwrap();

        for page in pages {
            for (block_idx, block) in page.blocks.iter().enumerate() {
                let keys = section_map.keys().cloned().collect::<Vec<String>>();
                let mut text_block = block.get_text().trim().to_string();

                // Get math-marked version if available
                let math_text = math_texts
                    .get(&(page.page_number, block_idx))
                    .cloned()
                    .unwrap_or_else(|| text_block.clone());
                let mut math_block = math_text.trim().to_string();

                if text_block.ends_with("-") {
                    last_text.push_str(&text_block.trim_end_matches("-"));
                    last_math_text.push_str(&math_block.trim_end_matches("-"));
                    continue;
                }

                if !last_text.is_empty() {
                    last_text.push_str(&text_block);
                    text_block = last_text.clone();
                    last_text.clear();

                    last_math_text.push_str(&math_block);
                    math_block = last_math_text.clone();
                    last_math_text.clear();
                }

                text_block = eos_ptn.replace_all(&text_block, "$1 $2").to_string();
                text_block = ex_ws_ptn.replace_all(&text_block, " ").to_string();
                math_block = eos_ptn.replace_all(&math_block, "$1 $2").to_string();
                math_block = ex_ws_ptn.replace_all(&math_block, " ").to_string();

                // Separate captions from body content
                let is_caption = block.block_type == BlockType::Caption;

                if is_caption {
                    caption_map
                        .entry(block.section.clone())
                        .or_insert_with(Vec::new)
                        .push(text_block);
                    if !section_indices.contains_key(&block.section) {
                        section_indices.insert(block.section.clone(), section_indices.len() as i16);
                    }
                } else {
                    if keys.contains(&block.section) {
                        section_map.get_mut(&block.section).unwrap().push(text_block);
                        math_section_map.get_mut(&block.section).unwrap().push(math_block);
                    } else {
                        section_map.insert(block.section.clone(), vec![text_block]);
                        math_section_map.insert(block.section.clone(), vec![math_block]);
                        section_indices.insert(block.section.clone(), section_indices.len() as i16);
                    }
                }
            }
        }

        let mut sections = Vec::new();
        for (title, contents) in section_map {
            let captions = caption_map.remove(&title).unwrap_or_default();
            let math_contents = math_section_map.remove(&title);

            // Only include math_contents if it differs from contents
            let has_math = math_contents.as_ref().map_or(false, |mc| {
                mc.iter().zip(contents.iter()).any(|(m, c)| m != c)
            });

            sections.push(Section {
                index: section_indices.get(&title).copied().unwrap_or(0),
                title: title,
                contents: contents,
                math_contents: if has_math { math_contents } else { None },
                captions: captions,
            });
        }

        for (title, captions) in caption_map {
            sections.push(Section {
                index: section_indices.get(&title).copied().unwrap_or(0),
                title: title,
                contents: Vec::new(),
                math_contents: None,
                captions: captions,
            });
        }

        sections.sort_by(|a, b| a.index.cmp(&b.index));
        return sections;
    }

    /// Returns the concatenated text of all `TextBlock` instances in the `Section`.
    ///
    /// # Returns
    ///
    /// A `String` containing the text of all contents in the section, separated by newlines.
    pub fn get_text(&self) -> String {
        if self.contents.len() == 0 {
            return String::new();
        } else {
            return self.contents.join("\n");
        }
    }

    /// Returns the concatenated math-marked text if available, otherwise regular text.
    ///
    /// # Returns
    ///
    /// A `String` containing the math-marked text if available, otherwise the regular text.
    pub fn get_math_text(&self) -> String {
        if let Some(ref math) = self.math_contents {
            if !math.is_empty() {
                return math.join("\n");
            }
        }
        self.get_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Suffix directly connected to a word should have hyphen inserted
    #[test]
    fn test_fix_suffix_hyphens_direct_connection() {
        // -based
        assert_eq!(fix_suffix_hyphens("databased"), "data-based");
        assert_eq!(fix_suffix_hyphens("modelbased"), "model-based");

        // -driven
        assert_eq!(fix_suffix_hyphens("eventdriven"), "event-driven");
        assert_eq!(fix_suffix_hyphens("datadriven"), "data-driven");

        // -oriented
        assert_eq!(fix_suffix_hyphens("objectoriented"), "object-oriented");

        // -aware
        assert_eq!(fix_suffix_hyphens("contextaware"), "context-aware");

        // -friendly
        assert_eq!(fix_suffix_hyphens("userfriendly"), "user-friendly");

        // -specific
        assert_eq!(fix_suffix_hyphens("domainspecific"), "domain-specific");
    }

    /// Test: Already hyphenated words should remain unchanged
    #[test]
    fn test_fix_suffix_hyphens_already_hyphenated() {
        assert_eq!(fix_suffix_hyphens("data-based"), "data-based");
        assert_eq!(fix_suffix_hyphens("event-driven"), "event-driven");
        assert_eq!(fix_suffix_hyphens("object-oriented"), "object-oriented");
        assert_eq!(fix_suffix_hyphens("context-aware"), "context-aware");
        assert_eq!(fix_suffix_hyphens("user-friendly"), "user-friendly");
        assert_eq!(fix_suffix_hyphens("domain-specific"), "domain-specific");
    }

    /// Test: Space-separated words should remain unchanged
    #[test]
    fn test_fix_suffix_hyphens_space_separated() {
        assert_eq!(fix_suffix_hyphens("data based"), "data based");
        assert_eq!(fix_suffix_hyphens("event driven"), "event driven");
        assert_eq!(fix_suffix_hyphens("object oriented"), "object oriented");
        assert_eq!(fix_suffix_hyphens("context aware"), "context aware");
    }

    /// Test: Multiple suffixes in one string
    #[test]
    fn test_fix_suffix_hyphens_multiple_occurrences() {
        assert_eq!(
            fix_suffix_hyphens("This is a databased and eventdriven system."),
            "This is a data-based and event-driven system."
        );
        assert_eq!(
            fix_suffix_hyphens("userfriendly and domainspecific approach"),
            "user-friendly and domain-specific approach"
        );
    }

    /// Test: Mixed cases (some need fixing, some don't)
    #[test]
    fn test_fix_suffix_hyphens_mixed_cases() {
        assert_eq!(
            fix_suffix_hyphens("data-based and eventdriven"),
            "data-based and event-driven"
        );
        assert_eq!(
            fix_suffix_hyphens("The modelbased approach is user-friendly."),
            "The model-based approach is user-friendly."
        );
    }

    /// Test: No suffix present - string should remain unchanged
    #[test]
    fn test_fix_suffix_hyphens_no_suffix() {
        assert_eq!(fix_suffix_hyphens("hello world"), "hello world");
        assert_eq!(fix_suffix_hyphens("simple text"), "simple text");
        assert_eq!(fix_suffix_hyphens(""), "");
    }

    /// Test: Suffix alone without prefix should remain unchanged
    #[test]
    fn test_fix_suffix_hyphens_suffix_alone() {
        // "based" alone requires at least one letter before it in the regex
        // so it should not match
        assert_eq!(fix_suffix_hyphens("based"), "based");
        assert_eq!(fix_suffix_hyphens("driven"), "driven");
        assert_eq!(fix_suffix_hyphens("oriented"), "oriented");
    }

    /// Test: All supported suffixes
    #[test]
    fn test_fix_suffix_hyphens_all_suffixes() {
        // Test a sampling of all suffix types
        let test_cases = vec![
            ("databased", "data-based"),
            ("datadriven", "data-driven"),
            ("objectoriented", "object-oriented"),
            ("contextaware", "context-aware"),
            ("platformagnostic", "platform-agnostic"),
            ("platformindependent", "platform-independent"),
            ("pathdependent", "path-dependent"),
            ("mobilefirst", "mobile-first"),
            ("cloudnative", "cloud-native"),
            ("datacentric", "data-centric"),
            ("resourceintensive", "resource-intensive"),
            ("cpubound", "cpu-bound"),
            ("threadsafe", "thread-safe"),
            ("errorfree", "error-free"),
            ("futureproof", "future-proof"),
            ("energyefficient", "energy-efficient"),
            ("codeoptimized", "code-optimized"),
            ("aienabled", "ai-enabled"),
            ("aipowered", "ai-powered"),
            ("productionready", "production-ready"),
            ("gpucapable", "gpu-capable"),
            ("backwardcompatible", "backward-compatible"),
            ("fullycompliant", "fully-compliant"),
            ("lowlevel", "low-level"),
            ("largescale", "large-scale"),
            ("systemwide", "system-wide"),
            ("taskspecific", "task-specific"),
            ("userfriendly", "user-friendly"),
            ("customerfacing", "customer-facing"),
            ("shelllike", "shell-like"),
            ("pythonstyle", "python-style"),
        ];

        for (input, expected) in test_cases {
            assert_eq!(
                fix_suffix_hyphens(input),
                expected,
                "Failed for input: {}",
                input
            );
        }
    }
}
