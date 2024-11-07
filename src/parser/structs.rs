use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub type PageNumber = i8;

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

    pub fn pdf_width(&self) -> i32 {
        return self.pdf_info.get("page_width").unwrap().parse::<i32>().unwrap();
    }
    pub fn pdf_height(&self) -> i32 {
        return self.pdf_info.get("page_height").unwrap().parse::<i32>().unwrap();
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub words: Vec<Word>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Line {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Line {
        Line {
            words: Vec::new(),
            x: x,
            y: y,
            width: width,
            height: height,
        }
    }
    pub fn add_word(&mut self, text: String, x: f32, y: f32, width: f32, height: f32) {
        self.words.push(Word {
            text: text.trim().to_string(),
            x: x,
            y: y,
            width: width,
            height: height,
        });
    }
    pub fn get_text(&self) -> String {
        let mut words = Vec::new();
        for word in &self.words {
            words.push(word.text.clone());
        }
        return words.join(" ");
    }
}

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
    pub fn add_line(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.lines.push(Line::new(x, y, width, height));
    }
    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text = text.trim_end_matches("- ").to_string();
            text.push_str(&line.get_text());
            text.push_str(" ");
        }
        return text;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub blocks: Vec<Block>,
    pub width: f32,
    pub height: f32,
    pub tables: Vec<Coordinate>,
    pub page_nubmer: PageNumber,
}

impl Page {
    pub fn new(width: f32, height: f32, page_number: PageNumber) -> Page {
        Page {
            blocks: Vec::new(),
            width: width,
            height: height,
            tables: Vec::new(),
            page_nubmer: page_number,
        }
    }

    pub fn add_block(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.blocks.push(Block::new(x, y, width, height));
    }

    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for block in &self.blocks {
            text.push_str(&block.get_text());
            text.push_str("\n\n");
        }
        return text;
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Point {
        Point { x: x, y: y }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Coordinate {
    pub top_left: Point,
    pub top_right: Point,
    pub bottom_left: Point,
    pub bottom_right: Point,
}

impl Coordinate {
    pub fn from_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> Coordinate {
        Coordinate {
            top_left: Point { x: x1, y: y1 },
            top_right: Point { x: x2, y: y1 },
            bottom_left: Point { x: x1, y: y2 },
            bottom_right: Point { x: x2, y: y2 },
        }
    }

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

    pub fn width(&self) -> f32 {
        return self.top_right.x - self.top_left.x;
    }

    pub fn height(&self) -> f32 {
        return self.bottom_left.y - self.top_left.y;
    }

    pub fn is_intercept(&self, other: &Coordinate) -> bool {
        if self.top_left.x >= other.bottom_right.x || self.bottom_right.x <= other.top_left.x {
            return false;
        }
        if self.top_left.y >= other.bottom_right.y || self.bottom_right.y <= other.top_left.y {
            return false;
        }
        return true;
    }

    pub fn get_area(&self) -> f32 {
        return self.width() * self.height();
    }

    pub fn intersection(&self, other: &Coordinate) -> Coordinate {
        let x1 = f32::max(self.top_left.x, other.top_left.x);
        let y1 = f32::max(self.top_left.y, other.top_left.y);
        let x2 = f32::min(self.bottom_right.x, other.bottom_right.x);
        let y2 = f32::min(self.bottom_right.y, other.bottom_right.y);
        return Coordinate::from_rect(x1, y1, x2, y2);
    }

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
    pub fn is_contained_in(&self, other: &Coordinate) -> bool {
        let iou = self.iou(other);
        let intersection = self.intersection(other).get_area();
        let self_area = self.get_area();
        return iou > 0.0 && intersection / self_area > 0.3;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub content: String,
}

impl Section {
    pub fn from_pages(pages: &Vec<Page>) -> Vec<Section> {
        let mut section_map: HashMap<String, Vec<String>> = HashMap::new();
        for page in pages {
            for block in &page.blocks {
                let keys = section_map.keys().cloned().collect::<Vec<String>>();
                if keys.contains(&block.section) {
                    let content = section_map.get_mut(&block.section).unwrap();
                    content.push(block.get_text().clone());
                } else {
                    section_map.insert(block.section.clone(), vec![block.get_text().clone()]);
                }
            }
        }
        let mut sections = Vec::new();
        for (title, content) in section_map {
            sections.push(Section {
                title: title,
                content: content.join("\n"),
            });
        }
        return sections;
    }
}
