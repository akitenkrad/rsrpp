use crate::parser::structs::*;
use anyhow::{Error, Result};
use glob::glob;
use opencv::core::{Vec4f, Vector};
use opencv::imgcodecs;
use opencv::imgproc;
use opencv::prelude::*;
use quick_xml::events::Event;
use reqwest as request;
use scraper::html;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

#[cfg(test)]
mod tests;

pub mod structs;

fn get_pdf_info(config: &mut ParserConfig) -> Result<()> {
    let res =
        Command::new("pdfinfo").args(&[config.pdf_path.clone()]).stdout(Stdio::piped()).output();
    let text = String::from_utf8(res?.stdout)?;
    for line in text.split("\n") {
        let parts: Vec<&str> = line.split(":").collect();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[0].trim().to_string().to_lowercase().replace(" ", "_");
        let value = parts[1].trim().to_string();

        if key == "page_size" {
            let regex = regex::Regex::new(r"(\d+) x (\d+)")?;
            let caps = regex.captures(&value).unwrap();
            config.pdf_info.insert("page_width".to_string(), caps[1].to_string());
            config.pdf_info.insert("page_height".to_string(), caps[2].to_string());
        }
        config.pdf_info.insert(key, value);
    }
    return Ok(());
}

fn save_pdf_as_figures(config: &mut ParserConfig) -> Result<()> {
    let pdf_path = Path::new(config.pdf_path.as_str());
    let dst_path = pdf_path.parent().unwrap().join(pdf_path.file_stem().unwrap().to_str().unwrap());

    // save pdf as jpeg files
    let res = Command::new("pdftocairo")
        .args(&[
            "-jpeg".to_string(),
            "-r".to_string(),
            "72".to_string(),
            pdf_path.to_str().unwrap().to_string(),
            dst_path.to_str().unwrap().to_string(),
        ])
        .stdout(Stdio::piped())
        .output();
    if let Err(e) = res {
        return Err(Error::msg(format!("Error: {}", e)));
    }

    // get all jpeg files
    let glob_query = dst_path.file_name().unwrap().to_str().unwrap().to_string() + "*.jpg";
    let glob_query = dst_path.parent().unwrap().join(glob_query);
    for entry in glob(glob_query.to_str().unwrap())? {
        match entry {
            Ok(path) => {
                let page_number: PageNumber = path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .split("-")
                    .last()
                    .unwrap()
                    .parse::<i8>()?;
                println!("page_number: {} - {}", page_number, path.to_str().unwrap());
                config.pdf_figures.insert(page_number, path.to_str().unwrap().to_string());
            }
            Err(e) => return Err(Error::msg(format!("Error: {}", e))),
        }
    }

    return Ok(());
}

fn save_pdf_as_xml(config: &mut ParserConfig) -> Result<()> {
    let xml_path = Path::new(&config.pdf_xml_path);

    Command::new("pdftohtml")
        .args(&[
            "-c".to_string(),
            "-s".to_string(),
            "-dataurls".to_string(),
            "-xml".to_string(),
            "-zoom".to_string(),
            "1.0".to_string(),
            config.pdf_path.as_str().to_string(),
            xml_path.to_str().unwrap().to_string(),
        ])
        .stdout(Stdio::piped())
        .output()?;

    // get title font size
    let mut font_number = 0;
    let xml_text = std::fs::read_to_string(xml_path)?;
    let mut reader = quick_xml::Reader::from_str(&xml_text);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"text" {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"font" {
                            font_number = String::from_utf8_lossy(attr.value.as_ref())
                                .parse::<i32>()
                                .unwrap();
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if String::from_utf8_lossy(e.as_ref()).to_lowercase() == "abstract" {
                    break;
                }
            }
            Err(_e) => {
                break;
            }
            _ => {}
        }
    }

    // get sections
    let mut page_number = 0;
    let mut is_title = false;
    let regex_is_number = regex::Regex::new(r"^\d+$").unwrap();
    let regex_trim_number = regex::Regex::new(r"\d\.").unwrap();
    let mut reader = quick_xml::Reader::from_str(&xml_text);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"page" {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"number" {
                            page_number =
                                String::from_utf8_lossy(attr.value.as_ref()).parse::<i8>().unwrap();
                        }
                    }
                } else if e.name().as_ref() == b"text" {
                    let _font_number = String::from_utf8_lossy(
                        e.attributes()
                            .find(|attr| attr.clone().unwrap().key.as_ref() == b"font")
                            .unwrap()
                            .unwrap()
                            .value
                            .as_ref(),
                    )
                    .parse::<i32>()
                    .unwrap();

                    if font_number == _font_number {
                        is_title = true;
                    } else {
                        is_title = false;
                    }
                    continue;
                }
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                if regex_is_number.is_match(&text) {
                    continue;
                }
                let text = regex_trim_number.replace(&text, "").to_string().trim().to_string();
                if is_title {
                    config.sections.push((page_number, text.to_string()));
                    if text.to_lowercase() == "references" {
                        break;
                    }
                }
            }
            Err(_e) => {
                break;
            }
            _ => {}
        }
    }

    return Ok(());
}

fn save_pdf_as_text(config: &mut ParserConfig) -> Result<()> {
    let html_path = Path::new(config.pdf_text_path.as_str());

    // parse pdf into html
    let _ = Command::new("pdftotext")
        .args(&[
            "-nopgbrk".to_string(),
            "-htmlmeta".to_string(),
            "-bbox-layout".to_string(),
            "-r".to_string(),
            "72".to_string(),
            config.pdf_path.as_str().to_string(),
            html_path.to_str().unwrap().to_string(),
        ])
        .stdout(Stdio::piped())
        .output()?;

    return Ok(());
}

async fn save_pdf(path_or_url: &str, config: &mut ParserConfig) -> Result<()> {
    let save_path = config.pdf_path.as_str();
    if path_or_url.starts_with("http") {
        let res = request::get(path_or_url).await;
        let bytes = res?.bytes().await;
        let out = File::create(save_path);
        std::io::copy(&mut bytes?.as_ref(), &mut out?)?;
    } else {
        let path = Path::new(path_or_url);
        let _ = std::fs::copy(path.as_os_str(), save_path);
    }

    // get pdf info
    get_pdf_info(config).unwrap();

    // save pdf as jpeg files
    save_pdf_as_figures(config).unwrap();

    // save pdf as html
    save_pdf_as_xml(config).unwrap();

    // save pdf as text
    save_pdf_as_text(config).unwrap();

    return Ok(());
}

async fn pdf2html(path_or_url: &str, config: &mut ParserConfig) -> Result<html::Html> {
    save_pdf(path_or_url, config).await?;

    let html_path = Path::new(config.pdf_text_path.as_str());

    let mut html = String::new();
    let mut f = File::open(html_path).expect("file not found");
    f.read_to_string(&mut html).expect("something went wrong reading the file");
    let html = scraper::Html::parse_document(&html);

    return Ok(html);
}

fn extract_tables(image_path: &str, tables: &mut Vec<Coordinate>, width: i32, height: i32) {
    // read the image
    let _src = imgcodecs::imread(image_path, imgcodecs::IMREAD_COLOR).unwrap();
    let mut src = Mat::zeros(width, height, _src.typ()).unwrap().to_mat().unwrap();

    let dst_size = opencv::core::Size::new(width, height);
    // reshape
    imgproc::resize(&_src, &mut src, dst_size, 0.0, 0.0, imgproc::INTER_LINEAR).unwrap();

    // convert the image to grayscale
    let mut src_gray = Mat::default();
    imgproc::cvt_color_def(&src, &mut src_gray, imgproc::COLOR_BGR2GRAY).unwrap();

    // apply Canny edge detector
    let mut edges = Mat::default();
    imgproc::canny_def(&src_gray, &mut edges, 50.0, 200.0).unwrap();

    // apply Hough Line Transform
    let min_line_length = src.size().unwrap().width as f64 / 10.0;
    let mut s_lines = Vector::<Vec4f>::new();
    imgproc::hough_lines_p(
        &edges,
        &mut s_lines,
        2.,
        PI / 180.,
        100,
        min_line_length,
        3.,
    )
    .unwrap();

    // extract tables
    let mut lines: Vec<(Point, Point)> = Vec::new();
    for s_line in s_lines {
        let [x1, y1, x2, y2] = *s_line;

        let a = (y2 - y1) / (x2 - x1);
        if a.abs() > 1e-2 {
            continue;
        }
        let len = ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt() as i32;
        if len < src.size().unwrap().width / 4 {
            continue;
        }
        let line = (Point::new(x1, y1), Point::new(x2, y2));
        lines.push(line);
    }

    let mut lines_gpd_by_len = HashMap::<i32, Vec<(Point, Point)>>::new();
    for line in lines {
        let mut len = ((line.0.x - line.1.x).powi(2) + (line.0.y - line.1.y).powi(2)).sqrt() as i32;
        for key in lines_gpd_by_len.keys() {
            if (len - key).abs() < 3 {
                len = *key;
                break;
            }
        }
        if !lines_gpd_by_len.contains_key(&len) {
            lines_gpd_by_len.insert(len, Vec::new());
        }
        lines_gpd_by_len.get_mut(&len).unwrap().push(line);
    }

    for line in lines_gpd_by_len.values() {
        if line.len() < 3 {
            continue;
        }
        let mut x_values: Vec<f32> = Vec::new();
        let mut y_values: Vec<f32> = Vec::new();
        for l in line {
            x_values.push(l.0.x);
            x_values.push(l.1.x);
            y_values.push(l.0.y);
            y_values.push(l.1.y);
        }
        x_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        y_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let x1 = x_values.first().unwrap().clone();
        let x2 = x_values.last().unwrap().clone();
        let y1 = y_values.first().unwrap().clone();
        let y2 = y_values.last().unwrap().clone();
        tables.push(Coordinate::from_rect(x1, y1, x2, y2));
    }
}

pub fn get_text_area(pages: &Vec<Page>) -> Coordinate {
    let mut left_values: Vec<f32> = Vec::new();
    let mut right_values: Vec<f32> = Vec::new();
    let mut top_values: Vec<f32> = Vec::new();
    let mut bottom_values: Vec<f32> = Vec::new();

    for page in pages {
        left_values.push(page.left());
        right_values.push(page.right());
        top_values.push(page.top());
        bottom_values.push(page.bottom());
    }

    let left = sci_rs::stats::median(left_values.iter()).0;
    let right = sci_rs::stats::median(right_values.iter()).0;
    let top = sci_rs::stats::median(top_values.iter()).0;
    let bottom = sci_rs::stats::median(bottom_values.iter()).0;

    return Coordinate {
        top_left: Point { x: left, y: top },
        top_right: Point { x: right, y: top },
        bottom_left: Point { x: left, y: bottom },
        bottom_right: Point {
            x: right,
            y: bottom,
        },
    };
}

fn adjst_columns(pages: &mut Vec<Page>, config: &ParserConfig) {
    let page_width = config.pdf_info.get("page_width").unwrap().parse::<f32>().unwrap();
    let half_width = page_width / 2.2;
    let last_page = config.sections.iter().map(|(page_number, _)| page_number).max().unwrap();
    let avg_line_width = pages
        .iter()
        .filter(|page| page.page_nubmer <= *last_page)
        .map(|page| {
            page.blocks
                .iter()
                .map(|block| {
                    block.lines.iter().map(|line| line.width).sum::<f32>()
                        / block.lines.len() as f32
                })
                .sum::<f32>()
                / page.blocks.len() as f32
        })
        .sum::<f32>()
        / pages.len() as f32;

    if avg_line_width < page_width / 1.5 {
        // Tow Columns
        for page in pages.iter_mut() {
            let mut right_blocks: Vec<Block> = Vec::new();
            let mut left_blocks: Vec<Block> = Vec::new();
            for block in page.blocks.iter() {
                if half_width < block.x {
                    right_blocks.push(block.clone());
                } else {
                    left_blocks.push(block.clone());
                }
            }
            left_blocks.append(&mut right_blocks);
            page.blocks = left_blocks;
        }
    }
}

pub async fn parse(path_or_url: &str, config: &mut ParserConfig) -> Result<Vec<Page>> {
    let html = pdf2html(path_or_url, config).await?;

    let mut pages = Vec::new();
    let page_selector = scraper::Selector::parse("page").unwrap();
    let _pages = html.select(&page_selector);
    for (_page_number, page) in _pages.enumerate() {
        let page_number = (_page_number + 1) as PageNumber;
        let page_width = page.value().attr("width").unwrap().parse::<f32>().unwrap();
        let page_height = page.value().attr("height").unwrap().parse::<f32>().unwrap();
        let mut _page = Page::new(page_width, page_height, page_number);

        // extract tables
        let fig_path = config.pdf_figures.get(&page_number).unwrap();
        extract_tables(
            fig_path,
            &mut _page.tables,
            _page.width as i32,
            _page.height as i32,
        );

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
            'line_iter: for line in _lines {
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

                for table in _page.tables.iter() {
                    let line_coord =
                        Coordinate::from_object(_line.x, _line.y, _line.width, _line.height);
                    if line_coord.is_contained_in(&table) {
                        continue 'line_iter;
                    }
                }

                let word_selector = scraper::Selector::parse("word").unwrap();
                let _words = line.select(&word_selector);
                for word in _words {
                    let word_xmin = word.value().attr("xmin").unwrap().parse::<f32>().unwrap();
                    let word_ymin = word.value().attr("ymin").unwrap().parse::<f32>().unwrap();
                    let word_xmax = word.value().attr("xmax").unwrap().parse::<f32>().unwrap();
                    let word_ymax = word.value().attr("ymax").unwrap().parse::<f32>().unwrap();
                    let text = word.text().collect::<String>();
                    _line.add_word(
                        text.clone(),
                        word_xmin,
                        word_ymin,
                        word_xmax - word_xmin,
                        word_ymax - word_ymin,
                    );
                }
                if _line.get_text().trim().len() > 0 {
                    _block.lines.push(_line);
                }
            }
            if _block.lines.len() > 0 {
                _page.blocks.push(_block);
            }
        }
        if _page.blocks.len() > 0 {
            pages.push(_page);
        }
    }

    adjst_columns(&mut pages, config);

    // set section for each block
    let mut current_section = "Abstract".to_string();
    let mut page_number = 1;
    let title_regex = regex::Regex::new(r"\d+\.").unwrap();
    for page in pages.iter_mut() {
        for block in page.blocks.iter_mut() {
            for line in block.lines.iter_mut() {
                let text = line.get_text();
                let text = title_regex.replace(&text, "").trim().to_string();
                if config.sections.iter().any(|(pg, section)| {
                    text.to_lowercase() == *section.to_lowercase() && pg == &page_number
                }) {
                    current_section = text;
                }
                block.section = current_section.clone();
            }
        }
        page_number += 1;
    }

    return Ok(pages);
}

pub fn pages2json(pages: &Vec<Page>) -> String {
    let sections = Section::from_pages(pages);
    let json = serde_json::to_string(&sections).unwrap();
    return json;
}
