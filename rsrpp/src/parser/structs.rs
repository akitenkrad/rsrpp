use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub type PageNumber = i8;

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
///
/// # Methods
///
/// * `new` - Creates a new instance of `ParserConfig` with default values.
/// * `pdf_width` - Returns the width of the PDF document as an `i32`.
/// * `pdf_height` - Returns the height of the PDF document as an `i32`.
/// * `clean_files` - Removes the PDF, text, XML, and figure files associated with the `ParserConfig`.
//
#[derive(Debug, Clone, PartialEq)]
pub struct ParserConfig {
    pub pdf_path: String,
    pub pdf_text_path: String,
    pub pdf_figures: HashMap<PageNumber, String>,
    pub pdf_xml_path: String,
    pub sections: Vec<(PageNumber, String)>,
    pub pdf_info: HashMap<String, String>,
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
        let mut rng = rand::thread_rng();
        let random_value = rng.gen_range(10000..99999);
        let mut pdf_path = String::new();
        pdf_path.push_str("/tmp/pdf_");
        pdf_path.push_str(&random_value.to_string());
        pdf_path.push_str(".pdf");

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
        }
    }

    /// Returns the width of the PDF page.
    ///
    /// This function retrieves the width of the PDF page from the `pdf_info` field,
    /// which is a `HashMap` containing additional information about the PDF.
    ///
    /// # Returns
    ///
    /// An `i32` representing the width of the PDF page.
    ///
    /// # Panics
    ///
    /// This function will panic if the `page_width` key is not found in the `pdf_info`
    /// `HashMap` or if the value cannot be parsed as an `i32`.
    pub fn pdf_width(&self) -> i32 {
        return self.pdf_info.get("page_width").unwrap().parse::<i32>().unwrap();
    }

    /// Returns the height of the PDF page.
    ///
    /// This function retrieves the height of the PDF page from the `pdf_info` field,
    /// which is a `HashMap` containing additional information about the PDF.
    ///
    /// # Returns
    ///
    /// An `i32` representing the height of the PDF page.
    ///
    /// # Panics
    ///
    /// This function will panic if the `page_height` key is not found in the `pdf_info`
    /// `HashMap` or if the value cannot be parsed as an `i32`.
    pub fn pdf_height(&self) -> i32 {
        return self.pdf_info.get("page_height").unwrap().parse::<i32>().unwrap();
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
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub lines: Vec<Line>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub section: String,
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
            text = text.trim().trim_end_matches("-").to_string();
            text.push_str(" ");
            text.push_str(&line.get_text());
        }
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
    pub page_nubmer: PageNumber,
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
            page_nubmer: page_number,
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
    /// A `f32` representing the y-coordinate of the topmost line.
    pub fn top(&self) -> f32 {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.y);
            }
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        return values.first().unwrap().clone();
    }

    /// Returns the y-coordinate of the bottommost line in the page.
    ///
    /// # Returns
    ///
    /// A `f32` representing the y-coordinate of the bottommost line.
    pub fn bottom(&self) -> f32 {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.y + line.height);
            }
        }
        values.sort_by(|a, b| b.partial_cmp(a).unwrap());
        return values.first().unwrap().clone();
    }

    /// Returns the x-coordinate of the leftmost line in the page.
    ///
    /// # Returns
    ///
    /// A `f32` representing the x-coordinate of the leftmost line.
    pub fn left(&self) -> f32 {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.x);
            }
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        return values.first().unwrap().clone();
    }

    /// Returns the x-coordinate of the rightmost line in the page.
    ///
    /// # Returns
    ///
    /// A `f32` representing the x-coordinate of the rightmost line.
    pub fn right(&self) -> f32 {
        let mut values: Vec<f32> = Vec::new();
        for block in &self.blocks {
            for line in &block.lines {
                values.push(line.x + line.width);
            }
        }
        values.sort_by(|a, b| b.partial_cmp(a).unwrap());
        return values.first().unwrap().clone();
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

pub struct Reference {
    pub text: String,
    pub coordinates: Coordinate,
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
/// * `title` - The title of the section.
/// * `content` - The content of the section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub index: i8,
    pub title: String,
    pub contents: Vec<String>,
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
    pub fn from_pages(pages: &Vec<Page>) -> Vec<Section> {
        let mut section_indices: HashMap<String, i8> = HashMap::new();
        let mut section_map: HashMap<String, Vec<String>> = HashMap::new();
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

                if keys.contains(&block.section) {
                    let content = section_map.get_mut(&block.section).unwrap();
                    content.push(text_block);
                } else {
                    section_map.insert(block.section.clone(), vec![text_block]);
                    section_indices.insert(block.section.clone(), section_indices.len() as i8);
                }
            }
        }
        let mut sections = Vec::new();
        for (title, contents) in section_map {
            sections.push(Section {
                index: section_indices.get(&title).unwrap().clone(),
                title: title,
                contents: contents,
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
}
