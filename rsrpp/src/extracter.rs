use crate::config::ParserConfig;
use crate::models::*;
use opencv::core::{Vec4f, Vector};
use opencv::imgcodecs;
use opencv::imgproc;
use opencv::prelude::*;
use std::collections::HashMap;
use std::f64::consts::PI;

pub fn extract_tables(image_path: &str, tables: &mut Vec<Coordinate>, width: i32, height: i32) {
    let _src = imgcodecs::imread(image_path, imgcodecs::IMREAD_COLOR).unwrap();
    let mut src = Mat::zeros(width, height, _src.typ()).unwrap().to_mat().unwrap();

    let dst_size = opencv::core::Size::new(width, height);
    imgproc::resize(&_src, &mut src, dst_size, 0.0, 0.0, imgproc::INTER_LINEAR).unwrap();

    let mut src_gray = Mat::default();
    imgproc::cvt_color_def(&src, &mut src_gray, imgproc::COLOR_BGR2GRAY).unwrap();

    let mut edges = Mat::default();
    imgproc::canny_def(&src_gray, &mut edges, 50.0, 200.0).unwrap();

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

pub fn adjst_columns(pages: &mut Vec<Page>, config: &ParserConfig) {
    let page_width = config.pdf_info.get("page_width").unwrap().parse::<f32>().unwrap();
    let last_page = config.sections.iter().map(|(page_number, _)| page_number).max().unwrap();
    let avg_line_width = pages
        .iter()
        .filter(|page| page.page_number <= *last_page)
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

    let half_width = page_width / 2.2;
    if avg_line_width < page_width / 1.5 {
        for page in pages.iter_mut() {
            page.number_of_columns = 2;
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

#[cfg(test)]
mod tests {
    use crate::config::ParserConfig;
    use crate::converter::pdf2html;
    use crate::extracter::adjst_columns;
    use crate::models::{Coordinate, Section};
    use crate::parser::parse_extract_textarea;
    use crate::parser::parse_html2pages;

    #[test]
    fn test_coordinate_is_intercept() {
        let a = Coordinate::from_rect(0.0, 0.0, 10.0, 10.0);
        let b = Coordinate::from_rect(5.0, 5.0, 15.0, 15.0);
        let c = Coordinate::from_rect(15.0, 15.0, 25.0, 25.0);
        let d = Coordinate::from_rect(0.0, 0.0, 5.0, 5.0);
        let e = Coordinate::from_rect(20.0, 5.0, 25.0, 10.0);
        let f = Coordinate::from_rect(5.0, 20.0, 10.0, 25.0);

        assert!(a.is_intercept(&b));
        assert!(!a.is_intercept(&c));
        assert!(a.is_intercept(&d));
        assert!(!a.is_intercept(&e));
        assert!(!a.is_intercept(&f));
        assert!(!b.is_intercept(&c));
        assert!(!b.is_intercept(&d));
        assert!(!b.is_intercept(&e));
        assert!(!b.is_intercept(&f));
    }

    #[tokio::test]
    async fn test_adjust_columns() {
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        let url = "https://arxiv.org/pdf/2411.19655";

        let html = pdf2html(url, &mut config, true, time).await.unwrap();

        let mut pages = parse_html2pages(&mut config, html).unwrap();

        parse_extract_textarea(&mut config, &mut pages).unwrap();

        adjst_columns(&mut pages, &mut config);

        tracing::info!("{}", &pages[0].number_of_columns);
        let sections = Section::from_pages(&pages);
        for section in sections.iter() {
            tracing::info!("{}: {}", section.title, section.get_text());
        }

        assert_eq!(pages[0].number_of_columns, 2);
    }
}
