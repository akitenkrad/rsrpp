use crate::config::ParserConfig;
use crate::models::*;
#[cfg(feature = "table-detection")]
use opencv::core::{Vec4f, Vector};
#[cfg(feature = "table-detection")]
use opencv::imgcodecs;
#[cfg(feature = "table-detection")]
use opencv::imgproc;
#[cfg(feature = "table-detection")]
use opencv::prelude::*;
#[cfg(feature = "table-detection")]
use std::f64::consts::PI;

/// How far a rule's left edge may sit from its cluster's reference edge, in points.
#[cfg(feature = "table-detection")]
const RULE_X_TOLERANCE: f32 = 5.0;

/// How far a rule's length may differ from its cluster's reference length, in points.
#[cfg(feature = "table-detection")]
const RULE_LEN_TOLERANCE: f32 = 3.0;

/// One near-horizontal rule, with its endpoints normalized left-to-right.
#[cfg(feature = "table-detection")]
struct Rule {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    points: (Point, Point),
}

#[cfg(feature = "table-detection")]
impl Rule {
    fn new(points: (Point, Point)) -> Rule {
        let (a, b) = (&points.0, &points.1);
        Rule {
            left: a.x.min(b.x),
            right: a.x.max(b.x),
            top: a.y.min(b.y),
            bottom: a.y.max(b.y),
            points,
        }
    }

    fn len(&self) -> f32 {
        self.right - self.left
    }
}

/// A run of rules that plausibly belong to the same table.
///
/// The cluster keeps its own reference edge, length and lowest extent, so membership is
/// decided against the cluster as a whole rather than against its most recent member.
/// That keeps the test order-independent and removes the repeated scan the previous
/// implementation needed to find the cluster's lowest rule.
#[cfg(feature = "table-detection")]
struct RuleCluster {
    left: f32,
    len: f32,
    max_y: f32,
    rules: Vec<(Point, Point)>,
}

#[cfg(feature = "table-detection")]
impl RuleCluster {
    fn new(rule: Rule) -> RuleCluster {
        RuleCluster {
            left: rule.left,
            len: rule.len(),
            max_y: rule.bottom,
            rules: vec![rule.points],
        }
    }

    fn accepts(&self, rule: &Rule, max_row_gap: f32) -> bool {
        (rule.len() - self.len).abs() < RULE_LEN_TOLERANCE
            && (rule.left - self.left).abs() < RULE_X_TOLERANCE
            && (rule.top - self.max_y).abs() < max_row_gap
    }

    fn push(&mut self, rule: Rule) {
        self.max_y = self.max_y.max(rule.bottom);
        self.rules.push(rule.points);
    }
}

/// Largest vertical gap between two rules that can still belong to the same table,
/// as a fraction of page height. Rules further apart than this start a new candidate.
#[cfg(feature = "table-detection")]
const MAX_TABLE_ROW_GAP_RATIO: f32 = 0.25;

/// Detect table regions on a page image so their text can be excluded from the body.
///
/// Without the `table-detection` feature this is a no-op: `tables` is left empty and
/// table text ends up in the body blocks. See the crate README for the trade-off.
#[cfg(not(feature = "table-detection"))]
pub fn extract_tables(_image_path: &str, _tables: &mut Vec<Coordinate>, _width: i32, _height: i32) {
}

#[cfg(feature = "table-detection")]
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

    // Group the rules into table candidates.
    //
    // Grouping by length alone (the original approach) merges rules that happen to be
    // the same width but sit in unrelated places — the left and right column of a
    // two-column paper, or a table and a chart on the same page. The bounding box of
    // such a group spans both columns and swallows the body text between them, which
    // is then discarded as "table content". Rules that belong to the same table share
    // their *horizontal extent*, not just their length, so cluster on the left edge as
    // well, and split a cluster when a vertical gap is too large to be a table's
    // interior ruling.
    let max_row_gap = height as f32 * MAX_TABLE_ROW_GAP_RATIO;

    let mut sorted_lines = lines;
    sorted_lines.sort_by(|a, b| a.0.y.partial_cmp(&b.0.y).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters: Vec<RuleCluster> = Vec::new();
    for line in sorted_lines {
        let candidate = Rule::new(line);

        // Compare against the cluster's own reference edge, never against whatever rule
        // happened to be added last: chaining the comparison lets a run of rules each
        // within tolerance of its predecessor drift arbitrarily far from the first, and
        // makes the result depend on the order the rules arrive in.
        match clusters.iter_mut().find(|cluster| cluster.accepts(&candidate, max_row_gap)) {
            Some(cluster) => cluster.push(candidate),
            None => clusters.push(RuleCluster::new(candidate)),
        }
    }

    let page_area = (width * height) as f32;

    for cluster in clusters.iter() {
        let line = &cluster.rules;
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
        x_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        y_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // x_values and y_values are guaranteed non-empty due to line.len() >= 3 check above
        if let (Some(&x1), Some(&x2), Some(&y1), Some(&y2)) = (
            x_values.first(),
            x_values.last(),
            y_values.first(),
            y_values.last(),
        ) {
            let table_coord = Coordinate::from_rect(x1, y1, x2, y2);
            // Skip table regions covering > 50% of page — likely false positives
            // from chart gridlines, figure borders, etc.
            if page_area > 0.0 && table_coord.get_area() / page_area > 0.5 {
                continue;
            }
            tracing::debug!(
                "Table region: x=[{:.0},{:.0}] y=[{:.0},{:.0}] ({:.0}x{:.0}, {:.1}% of page) from {} lines",
                x1,
                x2,
                y1,
                y2,
                x2 - x1,
                y2 - y1,
                table_coord.get_area() / page_area * 100.0,
                line.len()
            );
            tables.push(table_coord);
        }
    }
}

pub fn get_text_area(pages: &Vec<Page>) -> Coordinate {
    let mut left_values: Vec<f32> = Vec::new();
    let mut right_values: Vec<f32> = Vec::new();
    let mut top_values: Vec<f32> = Vec::new();
    let mut bottom_values: Vec<f32> = Vec::new();

    for page in pages {
        // Skip empty pages that have no lines
        if let (Some(left), Some(right), Some(top), Some(bottom)) =
            (page.left(), page.right(), page.top(), page.bottom())
        {
            left_values.push(left);
            right_values.push(right);
            top_values.push(top);
            bottom_values.push(bottom);
        }
    }

    let left = sci_rs::stats::median(left_values.iter()).0;
    let right = sci_rs::stats::median(right_values.iter()).0;
    let top = sci_rs::stats::median(top_values.iter()).0;
    let bottom = sci_rs::stats::median(bottom_values.iter()).0;

    let (page_width, page_height) =
        pages.first().map(|p| (p.width, p.height)).unwrap_or((595.0, 842.0));

    let area_width = right - left;
    let area_height = bottom - top;

    // If the computed text area is degenerate (too small relative to the page),
    // fall back to the full page dimensions to avoid filtering out all body blocks.
    if left_values.is_empty() || area_height < page_height * 0.2 || area_width < page_width * 0.2 {
        return Coordinate {
            top_left: Point { x: 0.0, y: 0.0 },
            top_right: Point {
                x: page_width,
                y: 0.0,
            },
            bottom_left: Point {
                x: 0.0,
                y: page_height,
            },
            bottom_right: Point {
                x: page_width,
                y: page_height,
            },
        };
    }

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

pub fn adjst_columns(pages: &mut Vec<Page>, config: &ParserConfig) -> anyhow::Result<()> {
    // Early return if no sections found - column adjustment is not possible
    let last_page = match config.sections.iter().map(|(page_number, _)| page_number).max() {
        Some(page) => *page,
        None => {
            tracing::warn!("No sections found, skipping column adjustment");
            return Ok(());
        }
    };

    let page_width = config
        .pdf_info
        .get("page_width")
        .ok_or_else(|| anyhow::anyhow!("page_width not available in pdf_info"))?
        .parse::<f32>()
        .map_err(|e| anyhow::anyhow!("Invalid page_width: {}", e))?;
    let avg_line_width = pages
        .iter()
        .filter(|page| page.page_number <= last_page)
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::ParserConfig;
    use crate::converter::pdf2html;
    use crate::extracter::adjst_columns;
    use crate::models::{Coordinate, Section};
    use crate::parser::parse_extract_textarea;
    use crate::parser::parse_html2pages;

    #[cfg(feature = "table-detection")]
    fn rule(left: f32, len: f32, y: f32) -> super::Rule {
        super::Rule::new((
            crate::models::Point::new(left, y),
            crate::models::Point::new(left + len, y),
        ))
    }

    /// A run of rules each within tolerance of its predecessor must not chain into one
    /// cluster: the cluster compares against its own reference edge, not its last member.
    #[cfg(feature = "table-detection")]
    #[test]
    fn test_rule_cluster_rejects_chained_horizontal_drift() {
        let mut cluster = super::RuleCluster::new(rule(0.0, 200.0, 10.0));
        // 4pt away — inside the 5pt tolerance, so it joins.
        let second = rule(4.0, 200.0, 20.0);
        assert!(cluster.accepts(&second, 200.0));
        cluster.push(second);

        // 8pt from the cluster's reference edge, though only 4pt from the last member.
        assert!(
            !cluster.accepts(&rule(8.0, 200.0, 30.0), 200.0),
            "drift past the tolerance must start a new cluster"
        );
    }

    #[cfg(feature = "table-detection")]
    #[test]
    fn test_rule_cluster_tracks_lowest_rule_for_the_gap_test() {
        let mut cluster = super::RuleCluster::new(rule(0.0, 200.0, 10.0));
        cluster.push(rule(0.0, 200.0, 100.0));

        // Measured from the cluster's lowest rule (y=100), not its first (y=10).
        assert!(cluster.accepts(&rule(0.0, 200.0, 150.0), 60.0));
        assert!(!cluster.accepts(&rule(0.0, 200.0, 200.0), 60.0));
    }

    #[cfg(feature = "table-detection")]
    #[test]
    fn test_rule_cluster_rejects_a_different_horizontal_extent() {
        // Same length, different column — the case that used to merge the two columns
        // of a two-column paper into one region and delete the body text between them.
        let cluster = super::RuleCluster::new(rule(72.0, 200.0, 100.0));
        assert!(!cluster.accepts(&rule(305.0, 200.0, 110.0), 200.0));
    }

    #[cfg(feature = "table-detection")]
    #[test]
    fn test_rule_normalizes_reversed_endpoints() {
        let forward = rule(100.0, 50.0, 10.0);
        let backward = super::Rule::new((
            crate::models::Point::new(150.0, 10.0),
            crate::models::Point::new(100.0, 10.0),
        ));
        assert_eq!(forward.left, backward.left);
        assert_eq!(forward.right, backward.right);
        assert_eq!(forward.len(), backward.len());
    }

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

        adjst_columns(&mut pages, &mut config).unwrap();

        tracing::info!("{}", &pages[0].number_of_columns);
        let sections = Section::from_pages(&pages);
        for section in sections.iter() {
            tracing::info!("{}: {}", section.title, section.get_text());
        }

        assert_eq!(pages[0].number_of_columns, 2);
    }
}
