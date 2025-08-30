use anyhow::Result;
use scraper::html;
use std::collections::HashMap;

use crate::config::{PageNumber, ParserConfig};
use crate::converter::pdf2html;
use crate::extracter::{adjst_columns, extract_tables, get_text_area};
use crate::models::{Block, Coordinate, Line, Page, Section};

pub(crate) fn parse_html2pages(config: &mut ParserConfig, html: html::Html) -> Result<Vec<Page>> {
    let mut pages = Vec::new();
    let page_selector = scraper::Selector::parse("page").unwrap();
    let _pages = html.select(&page_selector);
    for (_page_number, page) in _pages.enumerate() {
        let page_number = (_page_number + 1) as PageNumber;
        let page_width = page.value().attr("width").unwrap().parse::<f32>().unwrap();
        let page_height = page.value().attr("height").unwrap().parse::<f32>().unwrap();
        let mut _page = Page::new(page_width, page_height, page_number);

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
    return Ok(pages);
}

pub(crate) fn parse_extract_textarea(
    config: &mut ParserConfig,
    pages: &mut Vec<Page>,
) -> Result<()> {
    let section_titles =
        config.sections.iter().map(|(_, section)| section.to_lowercase()).collect::<Vec<String>>();
    let text_area = get_text_area(&pages);
    let title_index_regex = regex::Regex::new(r"\d+\.").unwrap();
    for page in pages.iter_mut() {
        let mut remove_indices: Vec<usize> = Vec::new();
        let width = if page.number_of_columns == 2 {
            page.width / 2.2
        } else {
            page.width / 1.1
        };
        for (i, block) in page.blocks.iter_mut().enumerate() {
            let block_coord = Coordinate::from_object(block.x, block.y, block.width, block.height);
            let iou = text_area.iou(&block_coord);
            let block_text = block.get_text();
            let block_text = title_index_regex.replace(&block_text, "").trim().to_string();

            if (iou - 0.0).abs() < 1e-6 {
                remove_indices.push(i);
            } else if !section_titles.contains(&block_text.to_lowercase())
                && (block.width / width < 0.3 && block.lines.len() < 4)
            {
                remove_indices.push(i);
            }
        }
        for i in remove_indices.iter().rev() {
            page.blocks.remove(*i);
        }
    }
    return Ok(());
}

pub(crate) fn parse_extract_section_text(
    config: &mut ParserConfig,
    pages: &mut Vec<Page>,
) -> Result<()> {
    let mut current_section = "Abstract".to_string();

    if cfg!(test) {
        tracing::info!("Initial section: {}", current_section);
    }

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
    return Ok(());
}
pub async fn parse(
    path_or_url: &str,
    config: &mut ParserConfig,
    verbose: bool,
) -> Result<Vec<Page>> {
    let time = std::time::Instant::now();
    if verbose {
        tracing::info!("Parsing PDF: {}", path_or_url);
    }

    let html = pdf2html(path_or_url, config, verbose, time).await?;
    if verbose {
        tracing::info!(
            "Converted PDF into HTML in {:.2}s",
            time.elapsed().as_secs()
        );
    }

    let mut pages = parse_html2pages(config, html)?;
    if verbose {
        tracing::info!(
            "Parsed HTML into Pages in {:.2}s, found {} pages",
            time.elapsed().as_secs(),
            pages.len()
        );
    }

    parse_extract_textarea(config, &mut pages)?;
    if verbose {
        tracing::info!("Extracted Text Area in {:.2}s", time.elapsed().as_secs());
    }

    adjst_columns(&mut pages, config);
    if verbose {
        tracing::info!("Adjusted Columns in {:.2}s", time.elapsed().as_secs());
    }

    parse_extract_section_text(config, &mut pages)?;
    if verbose {
        tracing::info!("Extracted Sections in {:.2}s", time.elapsed().as_secs());
    }

    if verbose {
        tracing::info!("Finished Parsing in {:.2}s", time.elapsed().as_secs());
    }

    return Ok(pages);
}

pub fn pages2json(pages: &Vec<Page>) -> String {
    let sections = Section::from_pages(pages);
    let mut json_data = Vec::<HashMap<&str, String>>::new();
    for section in sections.iter() {
        let mut data = HashMap::new();
        data.insert("title", section.title.clone());
        data.insert("contents", section.get_text());
        json_data.push(data);
    }
    let json = serde_json::to_string(&json_data).unwrap();
    return json;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::models::{Coordinate, Section};
    use crate::parser::pages2json;
    use crate::parser::parse;
    use crate::test_utils::{BuiltinPaper, TestPapers};

    #[test_log::test(tokio::test)]
    async fn test_parse_extract_sections_1() {
        let tp = TestPapers::setup().await.expect("setup test papers");
        let paper = tp.get_by_title(BuiltinPaper::AttentionIsAllYouNeed).unwrap();
        let mut config = ParserConfig::new();
        let mut pages = parse(
            paper.dest_path(&tp.tmp_dir).to_str().unwrap(),
            &mut config,
            true,
        )
        .await
        .unwrap();
        match parse_extract_section_text(&mut config, &mut pages) {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to extract sections: {}", e);
                assert!(false, "Failed to extract sections");
            }
        }

        for (idx, section) in config.sections.iter().enumerate() {
            tracing::info!("section {}: {}", idx, section.1);
        }
        assert!(config.sections.len() >= 5);
    }

    #[test_log::test(tokio::test)]
    async fn test_parse_extract_sections_2() {
        let tp = TestPapers::setup().await.expect("setup test papers");
        let paper = tp.get_by_title(BuiltinPaper::MemAgent).unwrap();
        let mut config = ParserConfig::new();
        let mut pages = parse(
            paper.dest_path(&tp.tmp_dir).to_str().unwrap(),
            &mut config,
            true,
        )
        .await
        .unwrap();
        match parse_extract_section_text(&mut config, &mut pages) {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to extract sections: {}", e);
                assert!(false, "Failed to extract sections");
            }
        }

        for (idx, section) in config.sections.iter().enumerate() {
            tracing::info!("section {}: {}", idx, section.1);
        }
        assert!(config.sections.len() >= 6);
    }

    #[test_log::test(tokio::test)]
    async fn test_parse_extract_sections_3() {
        let tp = TestPapers::setup().await.expect("setup test papers");
        let paper = tp.get_by_title(BuiltinPaper::UnsupervisedDialoguePolicies).unwrap();
        let mut config = ParserConfig::new();
        let mut pages = parse(
            paper.dest_path(&tp.tmp_dir).to_str().unwrap(),
            &mut config,
            true,
        )
        .await
        .unwrap();
        match parse_extract_section_text(&mut config, &mut pages) {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to extract sections: {}", e);
                assert!(false, "Failed to extract sections");
            }
        }

        for (idx, section) in config.sections.iter().enumerate() {
            tracing::info!("section {}: {}", idx, section.1);
        }
        assert!(config.sections.len() >= 9);
    }

    #[test_log::test(tokio::test)]
    async fn test_parse_1() {
        let mut config = ParserConfig::new();
        let url = "https://arxiv.org/pdf/2308.10379";
        let res = parse(url, &mut config, true).await;
        let pages = res.unwrap();

        assert!(pages.len() > 0, "No pages found");

        for page in pages {
            tracing::info!(
                "page: {}: ({}, {})",
                page.page_number,
                page.width,
                page.height
            );
            for block in &page.blocks {
                let block_coord =
                    Coordinate::from_object(block.x, block.y, block.width, block.height);
                tracing::info!(
                    "    {} [({},{})x({},{})]:{}",
                    block.section,
                    block_coord.top_left.x as i32,
                    block_coord.top_left.y as i32,
                    block_coord.bottom_right.x as i32,
                    block_coord.bottom_right.y as i32,
                    block.get_text()
                );
            }
        }

        let _ = config.clean_files();
    }

    #[test_log::test(tokio::test)]
    async fn test_parse_2() {
        let mut config = ParserConfig::new();
        let url = "https://arxiv.org/pdf/1706.03762";
        let res = parse(url, &mut config, true).await;
        let pages = res.unwrap();

        assert!(pages.len() > 0);

        for page in pages {
            tracing::info!(
                "page: {}: ({}, {})",
                page.page_number,
                page.width,
                page.height
            );
            for block in &page.blocks {
                let block_coord =
                    Coordinate::from_object(block.x, block.y, block.width, block.height);
                tracing::info!(
                    "    {} [({},{})x({},{})]:{}",
                    block.section,
                    block_coord.top_left.x as i32,
                    block_coord.top_left.y as i32,
                    block_coord.bottom_right.x as i32,
                    block_coord.bottom_right.y as i32,
                    block.get_text()
                );
            }
        }

        let _ = config.clean_files();
    }

    #[test_log::test(tokio::test)]
    async fn test_parse_local_sample() {
        let tp = TestPapers::setup().await.expect("setup test papers");
        // pick first paper
        let paper = &tp.papers[0];
        let path = paper.dest_path(&tp.tmp_dir);
        let mut config = ParserConfig::new();
        let pages =
            parse(path.to_str().unwrap(), &mut config, true).await.expect("parse local sample");
        assert!(pages.len() > 0, "No pages found");
        let _ = config.clean_files();
        let _ = tp.cleanup();
    }

    #[test_log::test(tokio::test)]
    async fn test_pdf_to_json() {
        let tp = TestPapers::setup().await.expect("setup test papers");

        for built_in_paper in BuiltinPaper::ALL.iter() {
            tracing::info!("Testing paper: {}", built_in_paper);
            let paper = tp.get_by_title(*built_in_paper).expect("paper not found");
            let mut config = ParserConfig::new();
            let filepath = paper.dest_path(&tp.tmp_dir);
            assert!(filepath.exists(), "file not found: {}", filepath.display());
            let pages = match parse(filepath.to_str().unwrap(), &mut config, true).await {
                Ok(pages) => pages,
                Err(e) => {
                    tracing::error!("Failed to parse {}: {}", paper.filename, e);
                    continue;
                }
            };
            let sections = Section::from_pages(&pages);
            assert!(sections.len() > 2, "At least 3 sections are expected");

            for section in sections.iter() {
                assert!(section.title.len() > 0);
                assert!(section.contents.len() > 0);
                tracing::info!("{}", section.title);
            }

            let json = serde_json::to_string(&sections).unwrap();
            assert!(json.len() > 0);

            let json = pages2json(&pages);
            assert!(json.len() > 0);
            let _ = config.clean_files();
        }
        let _ = tp.cleanup();
    }
}
