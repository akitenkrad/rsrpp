use anyhow::{Error, Result};
use rand::Rng;
use reqwest as request;
use scraper::html;
use scraper::Html;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

#[cfg(test)]
mod tests;

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
            text: text,
            x: x,
            y: y,
            width: width,
            height: height,
        });
    }
    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for word in &self.words {
            text.push_str(&word.text);
            text.push_str(" ");
        }
        return text;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub lines: Vec<Line>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Block {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Block {
        Block {
            lines: Vec::new(),
            x: x,
            y: y,
            width: width,
            height: height,
        }
    }
    pub fn add_line(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.lines.push(Line::new(x, y, width, height));
    }
    pub fn get_text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(&line.get_text());
            text.push_str("\n");
        }
        return text;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub blocks: Vec<Block>,
    pub width: f32,
    pub height: f32,
}

impl Page {
    pub fn new(width: f32, height: f32) -> Page {
        Page {
            blocks: Vec::new(),
            width: width,
            height: height,
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
}

async fn save_pdf(path_or_url: &str) -> Result<String> {
    let mut rng = rand::thread_rng();
    let random_value = rng.gen_range(10000..99999);
    let mut save_path = String::new();
    save_path.push_str("/tmp/pdf_");
    save_path.push_str(&random_value.to_string());
    save_path.push_str(".pdf");
    let save_path = save_path.as_str();
    if path_or_url.starts_with("http") {
        let res = request::get(path_or_url).await;
        if let Err(e) = res {
            return Err(Error::msg(format!("Error: {}", e)));
        };

        let bytes = res.unwrap().bytes().await;
        if let Err(e) = bytes {
            return Err(Error::msg(format!("Error: {}", e)));
        };

        let out = File::create(save_path);
        std::io::copy(&mut bytes.unwrap().as_ref(), &mut out.unwrap()).unwrap();

        return Ok(save_path.to_string());
    } else {
        let path = Path::new(path_or_url);
        let res = std::fs::copy(path.as_os_str(), save_path);
        if let Err(e) = res {
            return Err(Error::msg(format!("Error: {}", e)));
        }
    }

    return Ok(save_path.to_string());
}

pub async fn pdf2html(path: &str) -> Result<html::Html> {
    let result = save_pdf(path).await;
    if let Err(e) = result {
        return Err(e);
    }
    let save_path = result.unwrap();

    let html_path = Path::new(&save_path).with_extension("html");

    // parse pdf into html
    let res = Command::new("pdftotext")
        .args(&[
            save_path.to_string(),
            "-nopgbrk".to_string(),
            "-htmlmeta".to_string(),
            "-bbox-layout".to_string(),
            html_path.to_str().unwrap().to_string(),
        ])
        .stdout(Stdio::piped())
        .output();
    if let Err(e) = res {
        return Err(Error::msg(format!("Error: {}", e)));
    }

    let mut html = String::new();
    let mut f = File::open(html_path.clone()).expect("file not found");
    f.read_to_string(&mut html)
        .expect("something went wrong reading the file");
    let html = scraper::Html::parse_document(&html);

    if Path::new(save_path.as_str()).exists() {
        std::fs::remove_file(save_path).unwrap();
    }
    if html_path.exists() {
        std::fs::remove_file(html_path).unwrap();
    }

    return Ok(html);
}

pub fn parse_html(html: &Html) -> Result<Vec<Page>> {
    let mut pages = Vec::new();
    let page_selector = scraper::Selector::parse("page").unwrap();
    let _pages = html.select(&page_selector);
    for page in _pages {
        let page_width = page.value().attr("width").unwrap().parse::<f32>().unwrap();
        let page_height = page.value().attr("height").unwrap().parse::<f32>().unwrap();
        let mut _page = Page::new(page_width, page_height);

        let block_selector = scraper::Selector::parse("block").unwrap();
        let _blocks = page.select(&block_selector);
        for block in _blocks {
            let block_xmin = block.value().attr("xmin").unwrap().parse::<f32>().unwrap();
            let block_ymin = block.value().attr("ymin").unwrap().parse::<f32>().unwrap();
            let block_xmax = block.value().attr("xmax").unwrap().parse::<f32>().unwrap();
            let block_ymax = block.value().attr("ymax").unwrap().parse::<f32>().unwrap();
            let mut _block = Block::new(
                block_xmin,
                block_ymin,
                block_xmax - block_xmin,
                block_ymax - block_ymin,
            );

            let line_selector = scraper::Selector::parse("line").unwrap();
            let _lines = block.select(&line_selector);
            for line in _lines {
                let line_xmin = line.value().attr("xmin").unwrap().parse::<f32>().unwrap();
                let line_ymin = line.value().attr("ymin").unwrap().parse::<f32>().unwrap();
                let line_xmax = line.value().attr("xmax").unwrap().parse::<f32>().unwrap();
                let line_ymax = line.value().attr("ymax").unwrap().parse::<f32>().unwrap();
                let mut _line = Line::new(
                    line_xmin,
                    line_ymin,
                    line_xmax - line_xmin,
                    line_ymax - line_ymin,
                );

                let word_selector = scraper::Selector::parse("word").unwrap();
                let _words = line.select(&word_selector);
                for word in _words {
                    let word_xmin = word.value().attr("xmin").unwrap().parse::<f32>().unwrap();
                    let word_ymin = word.value().attr("ymin").unwrap().parse::<f32>().unwrap();
                    let word_xmax = word.value().attr("xmax").unwrap().parse::<f32>().unwrap();
                    let word_ymax = word.value().attr("ymax").unwrap().parse::<f32>().unwrap();
                    let text = word.text().collect::<String>();
                    _line.add_word(
                        text,
                        word_xmin,
                        word_ymin,
                        word_xmax - word_xmin,
                        word_ymax - word_ymin,
                    );
                }
                _block.lines.push(_line);
            }
            _page.blocks.push(_block);
        }
        pages.push(_page);
    }
    return Ok(pages);
}
