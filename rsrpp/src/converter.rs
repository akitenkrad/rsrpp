use crate::config::{PageNumber, ParserConfig};
use anyhow::{Error, Result};
use glob::glob;
use indicatif::ProgressBar;
use quick_xml::events::Event;
use regex::Regex;
use reqwest as request;
use scraper::html;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::LazyLock,
    time::Duration,
};

pub(crate) fn get_pdf_info(
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
    let res =
        Command::new("pdfinfo").args(&[config.pdf_path.clone()]).stdout(Stdio::piped()).output();
    let text = String::from_utf8(res?.stdout)?;

    if text.is_empty() {
        return Err(Error::msg("Error: pdf file is broken or invalid url"));
    }

    for line in text.split("\n") {
        let parts: Vec<&str> = line.split(":").collect();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[0].trim().to_string().to_lowercase().replace(" ", "_");
        let value = parts[1].trim().to_string();

        if key == "page_size" {
            let regex = regex::Regex::new(r"([\d|\.]+) x ([\d|\.]+).*?")?;
            if let Some(caps) = regex.captures(&value) {
                if let (Some(width), Some(height)) = (caps.get(1), caps.get(2)) {
                    config.pdf_info.insert("page_width".to_string(), width.as_str().to_string());
                    config.pdf_info.insert("page_height".to_string(), height.as_str().to_string());
                }
            }
        }
        config.pdf_info.insert(key, value);
    }

    if verbose {
        tracing::info!("Extracted PDF Info in {:.2}s", time.elapsed().as_secs());
    }
    return Ok(());
}

pub(crate) fn save_pdf_as_figures(
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
    let pdf_path = Path::new(config.pdf_path.as_str());
    let dst_path = pdf_path.parent().unwrap().join(pdf_path.file_stem().unwrap().to_str().unwrap());

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

    let glob_query = dst_path.file_name().unwrap().to_str().unwrap().to_string() + "*.jpg";
    let glob_query = dst_path.parent().unwrap().join(glob_query);

    let mut retry_count = 100;
    loop {
        let count = glob(glob_query.to_str().unwrap())?.count();
        if count > 0 {
            break;
        }
        if retry_count == 0 {
            return Err(Error::msg("Error: Failed to save PDF as JPEG files"));
        } else {
            std::thread::sleep(Duration::from_millis(100));
            retry_count -= 1;
        }
    }

    let glob_query_str =
        glob_query.to_str().ok_or_else(|| Error::msg("Invalid glob query path"))?;
    for entry in glob(glob_query_str)? {
        match entry {
            Ok(path) => {
                let page_number: PageNumber = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.split("-").last())
                    .ok_or_else(|| {
                        Error::msg(format!("Invalid figure filename format: {:?}", path))
                    })?
                    .parse::<PageNumber>()?;
                let path_str = path
                    .to_str()
                    .ok_or_else(|| Error::msg(format!("Invalid path encoding: {:?}", path)))?;
                config.pdf_figures.insert(page_number, path_str.to_string());
            }
            Err(e) => return Err(Error::msg(format!("Error: {}", e))),
        }
    }

    if verbose {
        tracing::info!(
            "Converted PDF as figures in {:.2}s",
            time.elapsed().as_secs()
        );
    }

    return Ok(());
}

pub(crate) fn save_pdf_as_xml(
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
    let xml_path = Path::new(&config.pdf_xml_path);

    let output = Command::new("pdftohtml")
        .args(&[
            "-c".to_string(),
            "-s".to_string(),
            "-xml".to_string(),
            "-zoom".to_string(),
            "1.0".to_string(),
            config.pdf_path.as_str().to_string(),
            xml_path.to_str().unwrap().to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::msg(format!(
            "pdftohtml failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        )));
    }

    let mut retry_count = 300;
    loop {
        if xml_path.exists() {
            break;
        }
        if retry_count == 0 {
            return Err(Error::msg("Error: Failed to save PDF as XML file"));
        } else {
            std::thread::sleep(Duration::from_secs(1));
            retry_count -= 1;

            if verbose {
                tracing::info!("Waiting for XML file... {}", retry_count);
            }
        }
    }

    let xml_text = std::fs::read_to_string(xml_path)?;
    detect_sections(config, &xml_text, verbose, time)?;

    return Ok(());
}

/// Finds the paper's section headings in the XML poppler produced and records them in
/// [`ParserConfig::sections`].
///
/// Split out from [`save_pdf_as_xml`] so that the heading rules can be exercised against
/// a hand-written XML fragment: reaching them through `save_pdf_as_xml` needs a real PDF
/// and a run of poppler, which is too coarse to pin a layout down to.
fn detect_sections(
    config: &mut ParserConfig,
    xml_text: &str,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
    // ── Step 1: Parse <fontspec> elements ──
    struct FontSpec {
        _id: i32,
        size: f32,
        family: String,
    }

    let mut font_specs: HashMap<i32, FontSpec> = HashMap::new();
    // Collect (font_id, char_count) for each <text> element
    let mut font_char_counts: HashMap<i32, usize> = HashMap::new();
    // Collect (font_id, lowercase_text) for anchor matching
    let mut font_texts: Vec<(i32, String)> = Vec::new();

    let anchor_words: &[&str] = &[
        "abstract",
        "introduction",
        "background",
        "related work",
        "method",
        "methodology",
        "methods",
        "experiments",
        "results",
        "discussion",
        "conclusion",
        "conclusions",
        "references",
        "acknowledgments",
        "acknowledgements",
        "appendix",
    ];

    // First pass: parse fontspec + collect font usage stats
    let mut current_font_id: i32 = 0;
    let mut current_text = String::new();
    let mut reader = quick_xml::Reader::from_str(&xml_text);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == "fontspec" {
                    let mut id = 0i32;
                    let mut size = 0.0f32;
                    let mut family = String::new();
                    for attr in e.attributes() {
                        let attr = attr?;
                        match attr.key.as_ref() {
                            "id" => {
                                id = attr.value.as_ref().parse::<i32>().unwrap_or(0);
                            }
                            "size" => {
                                size = attr.value.as_ref().parse::<f32>().unwrap_or(0.0);
                            }
                            "family" => {
                                family = attr.value.as_ref().to_string();
                            }
                            _ => {}
                        }
                    }
                    font_specs.insert(
                        id,
                        FontSpec {
                            _id: id,
                            size,
                            family,
                        },
                    );
                }
            }
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == "text" {
                    current_font_id = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.as_ref() == "font")
                        .map(|a| a.value.as_ref().parse::<i32>().unwrap_or(0))
                        .unwrap_or(0);
                    current_text.clear();
                }
            }
            Ok(Event::Text(e)) => {
                current_text.push_str(e.as_ref());
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == "text" {
                    let trimmed = current_text.trim();
                    let char_count = trimmed.chars().count();
                    if char_count > 0 {
                        *font_char_counts.entry(current_font_id).or_insert(0) += char_count;
                        font_texts.push((current_font_id, trimmed.to_lowercase()));
                    }
                    current_text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // ── Step 2: Determine body font size ──
    let body_font_id = font_char_counts.iter().max_by_key(|(_id, count)| *count).map(|(id, _)| *id);
    let body_font_size =
        body_font_id.and_then(|id| font_specs.get(&id)).map(|spec| spec.size).unwrap_or(0.0);

    if cfg!(test) {
        tracing::info!("Body font ID: {:?}, size: {}", body_font_id, body_font_size);
    }

    // ── Step 3: Score each font ──
    // Build set of font IDs that appear with anchor words
    let mut anchor_font_ids: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for (fid, text) in &font_texts {
        let t = text.trim();
        // Also check with leading number stripped (e.g., "1. Introduction" → "introduction")
        let stripped = regex::Regex::new(r"^\d+\.?\s*").unwrap().replace(t, "").to_string();
        if anchor_words.contains(&t) || anchor_words.contains(&stripped.as_str()) {
            anchor_font_ids.insert(*fid);
        }
    }

    let mut font_scores: HashMap<i32, f32> = HashMap::new();
    for (id, spec) in &font_specs {
        let mut score = 0.0f32;
        // Size comparison
        if body_font_size > 0.0 {
            if spec.size > body_font_size {
                score += 1.0;
            } else if spec.size < body_font_size {
                score -= 1.0;
            }
        }
        // Bold detection
        let family_lower = spec.family.to_lowercase();
        if family_lower.contains("bold") || family_lower.contains("black") {
            score += 0.3;
        }
        // Anchor word usage
        if anchor_font_ids.contains(id) {
            score += 0.5;
        }
        font_scores.insert(*id, score);
    }

    if cfg!(test) {
        for (id, score) in &font_scores {
            if let Some(spec) = font_specs.get(id) {
                tracing::info!(
                    "Font {}: size={}, family='{}', score={}",
                    id,
                    spec.size,
                    spec.family,
                    score
                );
            }
        }
    }

    // ── Step 4: Build title font set ──
    // Candidates: score >= 1.0
    let candidates: Vec<i32> =
        font_scores.iter().filter(|(_, score)| **score >= 1.0).map(|(id, _)| *id).collect();

    let title_font_set: std::collections::HashSet<i32>;
    let full_text_mode: bool;

    if candidates.is_empty() {
        // No candidates — fall back to full text mode
        tracing::warn!(
            "No section title fonts detected via scoring. \
             Using full text extraction mode for non-standard paper format."
        );
        full_text_mode = true;
        title_font_set = std::collections::HashSet::new();
    } else {
        // Find anchor-matched candidates to determine the canonical title size
        let anchor_candidates: Vec<i32> =
            candidates.iter().filter(|id| anchor_font_ids.contains(id)).copied().collect();

        // Among anchor candidates, pick the one with size closest to (but larger than)
        // the body font. Section headers are typically the smallest "larger-than-body" font,
        // not the largest (which is often the paper title).
        let best_anchor = anchor_candidates
            .iter()
            .filter_map(|&cid| font_specs.get(&cid).map(|s| (cid, s.size)))
            .filter(|(_, size)| *size > body_font_size)
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id);

        if let Some(anchor_id) = best_anchor.or(anchor_candidates.first().copied()) {
            // Use the size of the best anchor-matched font as canonical title size
            let title_size = font_specs.get(&anchor_id).map(|s| s.size).unwrap_or(0.0);
            // All fonts with that size among candidates → title font set
            title_font_set = candidates
                .iter()
                .filter(|id| {
                    font_specs.get(id).map(|s| (s.size - title_size).abs() < 0.1).unwrap_or(false)
                })
                .copied()
                .collect();
            full_text_mode = false;
        } else {
            // No anchor match among candidates; use all candidates
            title_font_set = candidates.into_iter().collect();
            full_text_mode = false;
        }

        if cfg!(test) {
            tracing::info!("Title font set: {:?}", title_font_set);
        }
    }

    if verbose || cfg!(test) {
        tracing::info!(
            "Font analysis completed in {:.2}s",
            time.elapsed().as_secs()
        );
    }

    // Skip section detection if in full text mode
    if full_text_mode {
        if verbose {
            tracing::info!("Skipping section detection - no title font detected");
        }
        return Ok(());
    }

    // ── Section detection pass ──
    let pb: Option<ProgressBar> = if verbose {
        let bar = ProgressBar::new(
            config.pdf_info.get("pages").unwrap_or(&String::from("0")).parse::<u64>().unwrap(),
        );
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/blue} {pos:>7}/{len:7} {msg}")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        Some(bar)
    } else {
        None
    };
    let mut page_number: PageNumber = 0;
    let mut start_paper = false;
    let mut start_paper_at: Option<usize> = None; // index in pending_sections when "abstract" was seen
    let mut probably_title = false;
    let mut pending_sections: Vec<PendingSection> = Vec::new();
    // Left and right edge of every body-font run long enough to be a line of prose.
    // The page's columns are read off these once the pass is over.
    let mut body_runs: Vec<(f32, f32)> = Vec::new();
    let mut heading: Option<OpenHeading> = None;
    let mut run_top: Option<f32> = None;
    let mut run_left: Option<f32> = None;
    let mut run_width: Option<f32> = None;
    let mut run_is_body = false;
    let regex_is_number = regex::Regex::new(r"^\d+$").unwrap();
    let regex_trim_number = regex::Regex::new(r"^\d+\.?\s*").unwrap();
    let mut reader = quick_xml::Reader::from_str(&xml_text);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == "page" {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == "number" {
                            page_number = attr.value.as_ref().parse::<PageNumber>().unwrap_or(0);
                        }
                    }
                } else if e.name().as_ref() == "text" {
                    let attr_of = |name: &str| -> Option<f32> {
                        e.attributes()
                            .filter_map(|attr| attr.ok())
                            .find(|attr| attr.key.as_ref() == name)
                            .and_then(|attr| attr.value.as_ref().parse::<f32>().ok())
                    };
                    let font_number = e
                        .attributes()
                        .filter_map(|attr| attr.ok())
                        .find(|attr| attr.key.as_ref() == "font")
                        .map(|attr| attr.value.as_ref().parse::<i32>().unwrap_or(0))
                        .unwrap_or(0);

                    probably_title = title_font_set.contains(&font_number);
                    run_is_body = body_font_id == Some(font_number);
                    run_top = attr_of("top");
                    run_left = attr_of("left");
                    run_width = attr_of("width");
                    continue;
                }
            }
            Ok(Event::Text(e)) => {
                let raw: &str = e.as_ref();

                if run_is_body && raw.chars().count() >= COLUMN_SAMPLE_CHARS {
                    if let (Some(left), Some(width)) = (run_left, run_width) {
                        body_runs.push((left, left + width));
                    }
                }

                // Decide whether this run continues the heading being assembled.
                //
                // Small-caps headings reach us in pieces: poppler emits "1", "I" in the
                // title font and "NTRODUCTION" in the body font, as three runs on one
                // line. Judging each piece on its own finds no section at all, so glue
                // runs back together while they stay on the same line and touch.
                //
                // Only within one page: `top` and `left` are page-relative, so a run at
                // the top of the next page can sit a few points from a run at the bottom
                // of this one and read as its continuation, welding two unrelated strings
                // into a heading that is in no page of the document.
                let continues_heading = match (heading.as_ref(), run_top, run_left) {
                    (Some(open), Some(top), Some(left)) => {
                        open.page == page_number
                            && (top - open.top).abs() <= HEADING_LINE_TOLERANCE
                            && (left - open.right).abs() <= HEADING_GAP_TOLERANCE
                    }
                    _ => false,
                };

                if continues_heading {
                    if let Some(open) = heading.as_mut() {
                        let left = run_left.unwrap_or(open.right);
                        // Word boundaries have to survive the join: the section text is
                        // later matched against the body text, where the words are spaced.
                        if left - open.right >= HEADING_SPACE_GAP && !open.text.ends_with(' ') {
                            open.text.push(' ');
                        }
                        open.text.push_str(raw);
                        open.right = left + run_width.unwrap_or(0.0);
                    }
                    continue;
                }

                // This run starts something new, so whatever was being assembled is done.
                flush_heading(
                    &mut heading,
                    &mut pending_sections,
                    &mut start_paper,
                    &mut start_paper_at,
                    &regex_is_number,
                    &regex_trim_number,
                );

                if let (true, Some(top), Some(left)) = (probably_title, run_top, run_left) {
                    heading = Some(OpenHeading {
                        page: page_number,
                        top,
                        left,
                        right: left + run_width.unwrap_or(0.0),
                        text: raw.to_string(),
                    });
                    continue;
                }

                // Not a heading: only the "Abstract" marker matters here.
                let text = regex_trim_number.replace(raw, "").trim().to_string();
                if text.to_lowercase() == "abstract" && start_paper_at.is_none() {
                    start_paper = true;
                    start_paper_at = Some(pending_sections.len());
                }
            }
            Ok(Event::Eof) => {
                flush_heading(
                    &mut heading,
                    &mut pending_sections,
                    &mut start_paper,
                    &mut start_paper_at,
                    &regex_is_number,
                    &regex_trim_number,
                );
                break;
            }
            Err(_e) => {
                break;
            }
            _ => {}
        }
    }

    tracing::debug!(
        "Section detection: start_paper={}, start_paper_at={:?}, {} title-font entries: {:?}",
        start_paper,
        start_paper_at,
        pending_sections.len(),
        pending_sections.iter().take(40).map(|s| &s.text).collect::<Vec<_>>()
    );

    // ── Placement pass ──
    // The font pass cannot tell a section heading from a subplot title or an axis label
    // set in the same size, and a false heading is not merely noise: it swallows the body
    // that follows, so the section it belongs to loses that text. Where the run sits on
    // the page settles what the font left open.
    let columns = detect_columns(&body_runs);
    // Under a handful of candidates there is not enough of the paper on show to read its
    // layout from, and one wrong verdict is then a large share of its sections.
    if !columns.is_empty() && pending_sections.len() >= PLACEMENT_MIN_HEADINGS {
        // Two passes. The first admits only candidates that answer to the page itself —
        // an anchor word, a column margin, a column centre. The second lets the rest
        // appeal to those, and to no one else, so that candidates of unknown standing
        // cannot vouch for each other.
        let established: Vec<PendingSection> = pending_sections
            .iter()
            .filter(|section| {
                heading_stands_on_its_own(section, &columns, anchor_words, &regex_trim_number)
            })
            .map(|section| PendingSection {
                page: section.page,
                left: section.left,
                right: section.right,
                text: section.text.clone(),
            })
            .collect();
        let keep: Vec<bool> = pending_sections
            .iter()
            .map(|section| {
                heading_is_placed_like_a_heading(
                    section,
                    &established,
                    &columns,
                    anchor_words,
                    &regex_trim_number,
                )
            })
            .collect();
        let rejected = keep.iter().filter(|k| !**k).count();
        // Rejecting most of the headings means the rule and the document disagree about
        // what this paper's layout is, and the document is the authority. Leave it alone.
        if rejected * 2 < keep.len() {
            for (section, kept) in pending_sections.iter().zip(keep.iter()) {
                if !kept {
                    tracing::debug!(
                        "Page {}: rejecting {:?} — left {:.0} matches no column margin, centre or heading indent",
                        section.page,
                        section.text,
                        section.left
                    );
                }
            }
            // `start_paper_at` indexes into this list, so it has to move with it.
            if let Some(index) = start_paper_at.as_mut() {
                *index = keep[..*index].iter().filter(|k| **k).count();
            }
            let mut kept_sections = Vec::with_capacity(keep.len() - rejected);
            for (section, keep) in pending_sections.into_iter().zip(keep) {
                if keep {
                    kept_sections.push(section);
                }
            }
            pending_sections = kept_sections;
        } else {
            tracing::debug!(
                "Skipping the placement pass: it rejects {} of {} headings, so the rule \
                 does not describe this layout",
                rejected,
                keep.len()
            );
        }
    }

    // Evaluate buffered sections after loop
    if start_paper {
        // Normal path: "Abstract" heading found — keep sections from that point onward.
        // start_paper_at records the pending_sections index at the moment "abstract" was seen.
        // If "abstract" was in title font, it's at that index; if not, the next title-font
        // entry starts there. This replicates the original behavior where start_paper=true
        // caused all subsequent title-font texts to be pushed directly.
        let skip = start_paper_at.unwrap_or(0);
        for section in pending_sections.into_iter().skip(skip) {
            config.sections.push((section.page, section.text));
        }
    } else if !pending_sections.is_empty() {
        // Fallback: no "Abstract" heading (e.g. Nature format)
        // Start from the first anchor-word match
        let first_anchor_idx = pending_sections.iter().position(|section| {
            let t = section.text.to_lowercase();
            let s = regex_trim_number.replace(&t, "").trim().to_string();
            anchor_words.iter().any(|&aw| aw == t.as_str() || aw == s.as_str())
        });

        if let Some(idx) = first_anchor_idx {
            // If the first anchor section is beyond page 1, infer an Abstract on page 1
            let first_page = pending_sections[idx].page;
            if first_page > 1 {
                config.sections.push((1, "Abstract".to_string()));
            }
            for section in pending_sections.into_iter().skip(idx) {
                config.sections.push((section.page, section.text));
            }
        }
    }

    if config.sections.is_empty() {
        tracing::warn!(
            "No sections detected despite finding title fonts. The document will be \
             returned as one undivided section — treat its structure as unreliable."
        );
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    if verbose {
        tracing::info!("Converted PDF into XML in {:.2}s", time.elapsed().as_secs());
    }

    return Ok(());
}

/// Vertical slack, in points, for deciding that two runs sit on the same line.
/// Small-caps continuations sit a couple of points lower than their initial capital.
const HEADING_LINE_TOLERANCE: f32 = 4.0;

/// Horizontal slack, in points, between the end of one run and the start of the next
/// before they stop counting as the same word or phrase.
const HEADING_GAP_TOLERANCE: f32 = 6.0;

/// Gap, in points, at which the join is a word boundary rather than a continuation of
/// the same word. Measured on small-caps headings: the pieces of one word sit 0-1pt
/// apart ("R" then "ELATED"), the space between words about 4pt.
const HEADING_SPACE_GAP: f32 = 3.0;

/// Longest text that is still plausibly a section heading. Anything longer is body
/// text that merely began with a run in a title font — an inline lead-in such as
/// "Training Details." — and must not become a section.
const HEADING_MAX_CHARS: usize = 80;

/// Whether `text` is a bare section label — "A", "1", "B." — carrying no title.
///
/// Appendix headings are laid out as a label and a title far enough apart that poppler
/// reports them separately, in the XML and in the body text alike. The title stands on
/// its own as the heading; the label would otherwise become a second, one-character
/// section holding nothing. Joining the two instead was tried and does not work: the
/// body text keeps them on separate lines, so a joined heading matches nothing there.
fn is_section_label(text: &str) -> bool {
    static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9]\.?$").unwrap());
    LABEL.is_match(text)
}

/// Whether `text` is a figure sub-label such as "(a)" or "(b) Refusal Rate".
///
/// These are set in the same font as the section headings of some papers, so the font
/// pass cannot tell them apart, and promoting one costs the real section its name: the
/// body that follows is filed under "(b)" instead of under the section it belongs to.
/// The parentheses are the giveaway — section labels never carry them.
fn is_figure_sublabel(text: &str) -> bool {
    static SUBLABEL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\(\s*[A-Za-z0-9]{1,3}\s*\)").unwrap());
    SUBLABEL.is_match(text)
}

/// Whether `text` is the arXiv stamp printed down the side of a preprint's first page.
///
/// It is set in its own font, larger than the body text, and it is rotated, so poppler
/// reports it at the very top of the page — ahead of the title. Left standing it becomes
/// the first heading of the paper and everything up to the next real heading is filed
/// under it: on eight of three hundred vault papers that was 327,000 characters, once an
/// entire 67,000-character paper.
fn is_arxiv_stamp(text: &str) -> bool {
    // The identifier has to be there. Without it the rule also rejects a heading that
    // merely names arXiv — "arXiv: A Large-Scale Dataset" is a title a paper about arXiv
    // could carry, and a section is a poor thing to lose to a prefix match.
    static STAMP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^arxiv[:\s]\s*(\d{4}\.\d{4,5}|[a-z-]+(\.[a-z]{2})?/\d{7})").unwrap()
    });
    STAMP.is_match(text)
}

/// A heading the font pass accepted, before the placement rules judge it.
struct PendingSection {
    page: PageNumber,
    /// Left edge of the first run, in points.
    left: f32,
    /// Right edge of the last run absorbed, in points.
    right: f32,
    text: String,
}

/// A text column of the page, as the body text draws it.
struct Column {
    /// Left margin, in points.
    margin: f32,
    /// Right edge, in points.
    right: f32,
}

impl Column {
    fn center(&self) -> f32 {
        (self.margin + self.right) / 2.0
    }
}

/// Shortest run that counts as a line of body text when the columns are measured.
/// Anything shorter is a caption fragment, a table cell or an axis label, and those
/// sit wherever the figure put them.
const COLUMN_SAMPLE_CHARS: usize = 40;

/// Slack, in points, for calling two left edges the same margin.
const MARGIN_TOLERANCE: f32 = 2.0;

/// Fewest heading candidates a document must offer before the placement pass will judge
/// them. Below this the paper has not shown enough of its layout to be held to it, and
/// a single wrong verdict would cost it a large share of its sections.
const PLACEMENT_MIN_HEADINGS: usize = 5;

/// Widest a cluster of left edges may span, in points, and still be read as one margin.
/// Wide enough to absorb the fringe poppler reports around a margin, narrow enough to
/// keep the two columns of a two-column paper apart.
const COLUMN_CLUSTER_SPAN: i32 = 5;

/// Slack, in points, for calling a heading centred on its column. Wider than the margin
/// tolerance because the centre is the midpoint of two measured edges, so it carries the
/// error of both, and because a centred heading is only ever centred to the eye.
const CENTER_TOLERANCE: f32 = 12.0;

/// Reads the page's text columns off the body text.
///
/// `runs` holds the left and right edge of every body-font run long enough to be a line
/// of prose. Their left edges pile up on the margins — one column or two, whichever the
/// paper uses — and everything else is scattered.
fn detect_columns(runs: &[(f32, f32)]) -> Vec<Column> {
    if runs.is_empty() {
        return Vec::new();
    }

    let mut histogram: HashMap<i32, usize> = HashMap::new();
    for (left, _) in runs {
        *histogram.entry(left.round() as i32).or_insert(0) += 1;
    }
    let mut bins: Vec<(i32, usize)> = histogram.into_iter().collect();
    bins.sort_by_key(|(left, _)| *left);

    // Cluster before counting, not after. One margin reaches poppler as several left
    // edges a point or two apart — justification, an italic first word, the rounding
    // itself — and a margin whose text is split over three such bins is a margin whose
    // bins may each fall under the threshold while their sum clears it easily. Measured
    // on seven arXiv papers, three of them have margins that are lost that way.
    let mut clusters: Vec<Vec<(i32, usize)>> = Vec::new();
    for bin in bins {
        // Against the cluster's last bin, which chains: left edges at 70, 75, 80, 85 join
        // into one cluster spanning fifteen points. Measuring from the first bin instead
        // is the tighter rule and was tried; over 300 vault papers it costs arXiv
        // P00001552 the headings of three real sections and returns one real section and
        // one table header elsewhere. A margin drawn a little too wide is cheap; a
        // section that never reaches the output is not.
        match clusters.last_mut() {
            Some(cluster) if (bin.0 - cluster.last().unwrap().0) <= COLUMN_CLUSTER_SPAN => {
                cluster.push(bin)
            }
            _ => clusters.push(vec![bin]),
        }
    }

    // A margin has to carry a real share of the body text. The floor of three keeps the
    // rule from reading a margin out of a two-line document.
    let minimum = std::cmp::max(3, runs.len() / 20);
    let mut columns: Vec<Column> = Vec::new();
    for cluster in clusters {
        if cluster.iter().map(|(_, count)| count).sum::<usize>() < minimum {
            continue;
        }
        // The cluster's busiest bin is the margin; the rest are its fringe.
        let left = cluster.iter().max_by_key(|(_, count)| *count).unwrap().0 as f32;
        // The right edge is measured over the same runs the count was taken from, so that
        // a cluster cannot clear the threshold on one set of lines and be measured on
        // another.
        let (first, last) = (cluster[0].0 as f32, cluster[cluster.len() - 1].0 as f32);
        let mut rights: Vec<f32> = runs
            .iter()
            .filter(|(run_left, _)| (first..=last).contains(&run_left.round()))
            .map(|(_, right)| *right)
            .collect();
        if rights.is_empty() {
            continue;
        }
        // The widest line overshoots on a hyphenated word or a wide formula; the ninth
        // decile is the column's right edge as the eye reads it.
        rights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        columns.push(Column {
            margin: left,
            right: rights[rights.len() * 9 / 10],
        });
    }
    columns
}

/// Whether `section` stands on its own as a heading, without leaning on another candidate.
///
/// A heading candidate is only ever a font judgement: the axis labels and legends of a
/// plot are often set larger than the body text, and `NIAH-Level 1` reads exactly like
/// `Related Work` on the page. Placement is what separates them. A heading starts at a
/// column margin or is centred on a column; a label inside a figure answers to the
/// figure, so it does neither.
///
/// Anchor words are exempt: `Abstract`, `Introduction` and the rest are as strong a
/// signal as any geometry, and one of them off on its own — a centred `Abstract` in a
/// paper whose sections are flush left — must not be lost to a placement rule.
fn heading_stands_on_its_own(
    section: &PendingSection,
    columns: &[Column],
    anchor_words: &[&str],
    trim_number: &regex::Regex,
) -> bool {
    let text = section.text.to_lowercase();
    let stripped = trim_number.replace(&text, "").trim().to_string();
    if anchor_words.contains(&text.as_str()) || anchor_words.contains(&stripped.as_str()) {
        return true;
    }
    if columns.iter().any(|column| (section.left - column.margin).abs() <= MARGIN_TOLERANCE) {
        return true;
    }
    let center = (section.left + section.right) / 2.0;
    columns.iter().any(|column| (center - column.center()).abs() <= CENTER_TOLERANCE)
}

/// Whether `section` sits where this paper puts a heading.
///
/// Numbered headings sit at an indent rather than at the margin — the number is at the
/// margin and the title beside it — so the indent has to be learned from the paper. It
/// is learned only from headings that already stand on their own, and only across pages:
/// judging a candidate by any other candidate lets two labels printed at the same spot
/// in two figures vouch for each other, which is the very thing this pass exists to
/// catch, and judging it by a candidate on its own page lets the four subplot titles of
/// one figure do the same.
fn heading_is_placed_like_a_heading(
    section: &PendingSection,
    established: &[PendingSection],
    columns: &[Column],
    anchor_words: &[&str],
    trim_number: &regex::Regex,
) -> bool {
    if heading_stands_on_its_own(section, columns, anchor_words, trim_number) {
        return true;
    }
    established.iter().any(|other| {
        other.page != section.page && (other.left - section.left).abs() <= MARGIN_TOLERANCE
    })
}

/// A heading being assembled from consecutive runs on one line.
struct OpenHeading {
    page: PageNumber,
    top: f32,
    /// Left edge of the first run, kept as the heading's own left through the joins.
    left: f32,
    /// Right edge of the last run absorbed, used to test adjacency of the next one.
    right: f32,
    text: String,
}

/// Finishes the heading under construction and records it.
///
/// Also notices "Abstract", which marks where the paper proper begins; with small-caps
/// headings that word only becomes visible once the runs have been glued back together.
fn flush_heading(
    heading: &mut Option<OpenHeading>,
    pending_sections: &mut Vec<PendingSection>,
    start_paper: &mut bool,
    start_paper_at: &mut Option<usize>,
    regex_is_number: &regex::Regex,
    regex_trim_number: &regex::Regex,
) {
    let Some(open) = heading.take() else {
        return;
    };

    let text = regex_trim_number.replace(open.text.trim(), "").trim().to_string();
    if text.is_empty()
        || regex_is_number.is_match(&text)
        || text.chars().count() > HEADING_MAX_CHARS
    {
        return;
    }
    // A label that never found a title, a figure sub-label, or the arXiv stamp printed
    // down the side of the page, is not a section.
    if is_section_label(&text) || is_figure_sublabel(&text) || is_arxiv_stamp(&text) {
        tracing::debug!(
            "Page {}: rejecting {:?} as a section heading",
            open.page,
            text
        );
        return;
    }

    if text.to_lowercase() == "abstract" && start_paper_at.is_none() {
        *start_paper = true;
        *start_paper_at = Some(pending_sections.len());
    }

    if cfg!(test) {
        tracing::info!("Found section title (p{}): {}", open.page, text);
    }
    pending_sections.push(PendingSection {
        page: open.page,
        left: open.left,
        right: open.right,
        text,
    });
}

pub(crate) fn save_pdf_as_text(
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
    let html_path = Path::new(config.pdf_text_path.as_str());

    let output = Command::new("pdftotext")
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
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::msg(format!(
            "pdftotext failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        )));
    }

    let mut retry_count = 300;
    loop {
        if html_path.exists() {
            break;
        } else if retry_count == 0 {
            return Err(Error::msg("Error: Failed to save PDF as text file"));
        } else {
            std::thread::sleep(Duration::from_secs(1));
            retry_count -= 1;

            if verbose {
                tracing::info!("Waiting for text file... {}", retry_count);
            }
        }
    }

    if verbose {
        tracing::info!(
            "Converted PDF into Text in {:.2}s",
            time.elapsed().as_secs()
        );
    }

    return Ok(());
}

pub(crate) async fn save_pdf(
    path_or_url: &str,
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
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

    get_pdf_info(config, verbose, time)?;

    save_pdf_as_figures(config, verbose, time)?;

    save_pdf_as_xml(config, verbose, time)?;

    save_pdf_as_text(config, verbose, time)?;

    return Ok(());
}

pub async fn pdf2html(
    path_or_url: &str,
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<html::Html> {
    save_pdf(path_or_url, config, verbose, time).await?;

    let html_path = Path::new(config.pdf_text_path.as_str());

    let mut html = String::new();
    let mut f = File::open(html_path).expect("file not found");
    f.read_to_string(&mut html).expect("something went wrong reading the file");
    let html = scraper::Html::parse_document(&html);

    return Ok(html);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::test_utils::{BuiltinPaper, TestPapers};

    #[test]
    fn test_is_section_label_matches_only_a_lone_label() {
        for label in ["A", "B.", "1", "9."] {
            assert!(is_section_label(label), "{label} should read as a label");
        }
        // The first word of a real heading must not: it would lose its own text.
        for heading in ["The", "LLM", "AoT", "Abstract", "A Survey", ""] {
            assert!(
                !is_section_label(heading),
                "{heading:?} should not read as a label"
            );
        }
    }

    /// XML in the shape poppler emits: one body font carrying most of the characters,
    /// one larger bold font for the headings.
    fn xml_with_pages(pages: &[&str]) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<pdf2xml>\n");
        xml.push_str("<fontspec id=\"0\" size=\"10\" family=\"Times\"/>\n");
        xml.push_str("<fontspec id=\"1\" size=\"12\" family=\"Times-Bold\"/>\n");
        for (index, body) in pages.iter().enumerate() {
            xml.push_str(&format!(
                "<page number=\"{}\" position=\"absolute\" top=\"0\" left=\"0\" height=\"792\" width=\"612\">\n{}\n</page>\n",
                index + 1,
                body
            ));
        }
        xml.push_str("</pdf2xml>\n");
        xml
    }

    #[test]
    fn test_detect_sections_does_not_join_a_heading_across_pages() {
        // `top` and `left` are page-relative, so a run at the top of one page can land
        // within a few points of one at the bottom of the previous page and read as its
        // continuation. Welding the two produces a heading that is on no page at all,
        // and the real heading on the second page loses its name.
        let xml = xml_with_pages(&[
            concat!(
                "<text top=\"100\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "<text top=\"120\" left=\"72\" width=\"400\" height=\"10\" font=\"0\">",
                "We describe a memory agent trained with reinforcement learning over ",
                "multiple conversations, and evaluate it on long-context benchmarks.",
                "</text>\n",
                // An appendix label left open at the foot of the page.
                "<text top=\"700\" left=\"72\" width=\"8\" height=\"12\" font=\"1\">A</text>"
            ),
            concat!(
                "<text top=\"702\" left=\"82\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"722\" left=\"72\" width=\"400\" height=\"10\" font=\"0\">",
                "Language models forget what they read.</text>"
            ),
        ]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            titles.contains(&"Introduction"),
            "the heading on page 2 must stand on its own, got {:?}",
            titles
        );
        assert!(
            !titles.iter().any(|t| t.contains("AIntroduction")),
            "a heading must not span two pages, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_still_joins_small_caps_on_one_line() {
        // The join exists for small caps: poppler splits "INTRODUCTION" into an initial
        // in the title font and the rest beside it. Guard that the page check left it
        // working.
        let xml = xml_with_pages(&[concat!(
            "<text top=\"100\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
            "<text top=\"120\" left=\"72\" width=\"400\" height=\"10\" font=\"0\">",
            "We describe a memory agent trained with reinforcement learning over ",
            "multiple conversations, and evaluate it on long-context benchmarks.",
            "</text>\n",
            "<text top=\"200\" left=\"72\" width=\"8\" height=\"12\" font=\"1\">I</text>\n",
            "<text top=\"201\" left=\"80\" width=\"80\" height=\"12\" font=\"1\">NTRODUCTION</text>"
        )]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            titles.contains(&"INTRODUCTION"),
            "the small-caps heading must be reassembled, got {:?}",
            titles
        );
    }

    /// Four lines of body text at one margin, enough for the column pass to see it.
    fn body_column(top: f32) -> String {
        (0..4)
            .map(|i| {
                format!(
                    "<text top=\"{}\" left=\"72\" width=\"400\" height=\"10\" font=\"0\">\
                     Language models forget what they read, and the memory agent has to \
                     decide what to keep.</text>",
                    top + i as f32 * 12.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_detect_sections_rejects_a_label_inside_a_figure() {
        // A subplot title is set in the heading font and reads like a heading —
        // "NIAH-Level 1" is as plausible as "Related Work" — but it sits where the
        // figure put it, at no column margin and at no other heading's position. Left
        // standing it becomes a section and takes the body that follows with it.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"260\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Related Work</text>\n",
                "<text top=\"320\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>\n",
                "<text top=\"360\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Conclusion</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(titles.contains(&"Introduction"), "got {:?}", titles);
        assert!(
            !titles.contains(&"NIAH-Level 1"),
            "a label adrift in a figure is not a section, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_keeps_a_heading_centred_on_the_column() {
        // Some papers centre their headings, so each one starts at a different left and
        // none starts at the margin. The centre is what they share.
        let column_center = (72.0 + 472.0) / 2.0;
        let heading_left = column_center - 50.0;
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"{}\" width=\"100\" height=\"12\" font=\"1\">Study 1</text>"
            ),
            body_column(80.0),
            heading_left
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            titles.contains(&"Study 1"),
            "a centred heading must survive, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_rejects_labels_that_only_vouch_for_each_other() {
        // The same subplot title printed at the same spot on two pages. Judging one
        // candidate by another of unknown standing lets the pair carry each other past
        // the pass that exists to catch them; only headings that answer to the page —
        // an anchor word, a margin, a centre — may be appealed to.
        let label = "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>";
        let xml = xml_with_pages(&[
            format!(
                concat!(
                    "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                    "{}\n",
                    "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                    "<text top=\"260\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Related Work</text>\n",
                    "<text top=\"320\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>\n",
                    "{}"
                ),
                body_column(80.0),
                label
            )
            .as_str(),
            format!(
                concat!(
                    "{}\n",
                    "<text top=\"300\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Conclusion</text>\n",
                    "{}"
                ),
                body_column(60.0),
                label
            )
            .as_str(),
        ]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(titles.contains(&"Conclusion"), "got {:?}", titles);
        assert!(
            !titles.contains(&"NIAH-Level 1"),
            "two labels must not vouch for each other, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_learns_the_indent_of_numbered_headings() {
        // Numbered headings sit at an indent, not at the margin: the number is at the
        // margin and the title beside it. An appendix heading at that indent, with no
        // anchor word of its own, is carried by the headings that established it.
        let xml = xml_with_pages(&[
            format!(
                concat!(
                    "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                    "{}\n",
                    "<text top=\"200\" left=\"91\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                    "<text top=\"260\" left=\"91\" width=\"90\" height=\"12\" font=\"1\">Related Work</text>\n",
                    "<text top=\"320\" left=\"91\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>"
                ),
                body_column(80.0)
            )
            .as_str(),
            format!(
                concat!(
                    "{}\n",
                    "<text top=\"300\" left=\"91\" width=\"140\" height=\"12\" font=\"1\">Implementation Details</text>"
                ),
                body_column(60.0)
            )
            .as_str(),
        ]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            titles.contains(&"Implementation Details"),
            "the appendix heading shares the indent of the numbered ones, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_stands_down_on_a_document_with_few_headings() {
        // Three candidates is not enough of a layout to hold the paper to, and rejecting
        // one of them would cost it a third of its sections.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">Findings</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            titles.contains(&"Findings"),
            "the pass must not judge a document this small, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_stands_down_one_candidate_short() {
        // Four candidates — the last count at which the pass leaves the document alone.
        // The label sits where no column margin or centre is, so the pass would reject it
        // if it ran; that it survives is what says the pass stood down.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"320\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(titles.len(), 4, "four candidates is the boundary, got {:?}", titles);
        assert!(
            titles.contains(&"NIAH-Level 1"),
            "one candidate short of the threshold, the pass must not run, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_judges_a_document_at_the_threshold() {
        // Five candidates — the first count at which the pass runs, and the same figure
        // label is now removed. Six is covered by
        // `test_detect_sections_rejects_a_label_inside_a_figure`.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"260\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Related Work</text>\n",
                "<text top=\"320\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(titles.contains(&"Experiments"), "got {:?}", titles);
        assert!(
            !titles.contains(&"NIAH-Level 1"),
            "at five candidates the pass runs, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_removes_two_of_five_candidates() {
        // Two rejections out of five: still a minority, so the verdicts stand and both
        // labels go.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"320\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>\n",
                "<text top=\"430\" left=\"404\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 2</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert!(titles.contains(&"Experiments"), "got {:?}", titles);
        assert!(
            !titles.contains(&"NIAH-Level 1") && !titles.contains(&"NIAH-Level 2"),
            "two of five is a minority, so both labels go, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_sections_stands_down_when_it_would_reject_three_of_five() {
        // Three rejections out of five is no longer a minority. The rule and the document
        // disagree about the layout, and the document is the authority, so every candidate
        // is kept — including the labels the pass would have removed one fewer ago.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>\n",
                "<text top=\"430\" left=\"404\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 2</text>\n",
                "<text top=\"460\" left=\"428\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 3</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(titles.len(), 5, "nothing may be dropped, got {:?}", titles);
        assert!(
            titles.contains(&"NIAH-Level 3"),
            "rejecting the majority means the pass does not describe this layout, got {:?}",
            titles
        );
    }


    #[test]
    fn test_detect_sections_stands_down_on_an_even_split() {
        // Three of six is exactly half, not a majority, and the pass stands down here too:
        // half the candidates disagreeing with the rule is already the document telling us
        // the rule has the layout wrong. This is the case that separates `<` from `<=`.
        let xml = xml_with_pages(&[format!(
            concat!(
                "<text top=\"60\" left=\"72\" width=\"50\" height=\"12\" font=\"1\">Abstract</text>\n",
                "{}\n",
                "<text top=\"200\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Introduction</text>\n",
                "<text top=\"320\" left=\"72\" width=\"90\" height=\"12\" font=\"1\">Experiments</text>\n",
                "<text top=\"400\" left=\"380\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 1</text>\n",
                "<text top=\"430\" left=\"404\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 2</text>\n",
                "<text top=\"460\" left=\"428\" width=\"60\" height=\"12\" font=\"1\">NIAH-Level 3</text>"
            ),
            body_column(80.0)
        )
        .as_str()]);

        let mut config = ParserConfig::new();
        detect_sections(&mut config, &xml, false, std::time::Instant::now()).unwrap();

        let titles: Vec<&str> = config.sections.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(titles.len(), 6, "an even split drops nothing, got {:?}", titles);
        assert!(
            titles.contains(&"NIAH-Level 3"),
            "half is not a majority, so the pass stands down, got {:?}",
            titles
        );
    }

    #[test]
    fn test_detect_columns_merges_a_margin_split_across_bins() {
        // One margin reaches poppler as a few left edges a point or two apart. Counting
        // the bins separately can leave each under the threshold while their sum clears
        // it, and the margin is then never found at all.
        let mut runs: Vec<(f32, f32)> = Vec::new();
        for _ in 0..400 {
            runs.push((71.0, 291.0));
        }
        // 21 runs at one margin, spread over three bins. The threshold is 21, so every
        // bin is under it on its own and the cluster is exactly on it.
        for (left, times) in [(77.0, 8), (78.0, 7), (80.0, 6)] {
            for _ in 0..times {
                runs.push((left, 291.0));
            }
        }

        let columns = detect_columns(&runs);
        let margins: Vec<f32> = columns.iter().map(|c| c.margin).collect();
        assert_eq!(
            margins,
            vec![71.0, 77.0],
            "the split margin must survive as its busiest bin, got {:?}",
            margins
        );
    }

    #[test]
    fn test_detect_columns_joins_a_run_of_close_bins() {
        // Left edges every five points chain into one margin. That is deliberate, not an
        // oversight: measured over 300 vault papers, splitting them costs more sections
        // than it saves. The right edge is measured over the whole chain, so the margin
        // is described by the same lines that were counted for it.
        let mut runs: Vec<(f32, f32)> = Vec::new();
        for (left, times) in [(70.0, 9), (75.0, 9), (80.0, 9), (85.0, 9)] {
            for _ in 0..times {
                runs.push((left, 291.0));
            }
        }

        let columns = detect_columns(&runs);
        let margins: Vec<f32> = columns.iter().map(|c| c.margin).collect();
        assert_eq!(
            margins.len(),
            1,
            "a run of close bins is one margin: {:?}",
            margins
        );
    }

    #[test]
    fn test_detect_columns_reads_the_margins_off_the_body_text() {
        // Two columns, and a scatter of figure text that must not become a third.
        let mut runs: Vec<(f32, f32)> = Vec::new();
        for _ in 0..20 {
            runs.push((71.0, 291.0));
            runs.push((306.0, 526.0));
        }
        runs.push((188.0, 240.0));
        runs.push((417.0, 460.0));

        let columns = detect_columns(&runs);
        let margins: Vec<f32> = columns.iter().map(|c| c.margin).collect();
        assert_eq!(margins, vec![71.0, 306.0], "got {:?}", margins);
    }

    #[test]
    fn test_is_arxiv_stamp_matches_the_side_stamp() {
        for stamp in [
            "arXiv:2607.00911v1  [cs.SE]  1 Jul 2026",
            "arXiv:1802.09089v2 [cs.CR] 27 May 2018",
            "arXiv:cs/0701001v1",
            "ARXIV 2606.12828",
        ] {
            assert!(is_arxiv_stamp(stamp), "{stamp:?} should read as the stamp");
        }
        for heading in [
            "Abstract",
            "Introduction",
            "Archival Research",
            "A Survey",
            // Names arXiv, but carries no identifier: a heading, not the stamp.
            "arXiv: A Large-Scale Dataset of Preprints",
            "arXiv Search and URLs",
        ] {
            assert!(
                !is_arxiv_stamp(heading),
                "{heading:?} should not read as the stamp"
            );
        }
    }

    #[test]
    fn test_is_figure_sublabel_matches_parenthesised_labels() {
        // "(b)" used to become a section and take the body of the real one with it.
        for sublabel in ["(a)", "(b) Refusal Rate", "( c )", "(1) Overview"] {
            assert!(
                is_figure_sublabel(sublabel),
                "{sublabel:?} should read as a sub-label"
            );
        }
        for heading in [
            "Conclusion",
            "A",
            "Results (final)",
            "(experimental) setup x",
        ] {
            assert!(
                !is_figure_sublabel(heading),
                "{heading:?} should not read as a sub-label"
            );
        }
    }

    #[test_log::test(tokio::test)]
    async fn test_pdf2html_url() {
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        let url = "https://arxiv.org/pdf/1706.03762";
        let res = pdf2html(url, &mut config, true, time).await;
        let html = res.unwrap();
        assert!(html.html().contains("arXiv:1706.03762"));
        let _ = config.clean_files();
    }

    #[test_log::test(tokio::test)]
    async fn test_pdf2html_file() {
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        let url = "https://arxiv.org/pdf/1706.03762";
        let response = request::get(url).await.unwrap();
        let bytes = response.bytes().await.unwrap();
        let path = "/tmp/test.pdf";
        let mut file = File::create(path).unwrap();
        std::io::copy(&mut bytes.as_ref(), &mut file).unwrap();

        let res = pdf2html("/tmp/test.pdf", &mut config, true, time).await;
        let html = res.unwrap();
        assert!(html.html().contains("arXiv:1706.03762"));

        let _ = config.clean_files();
    }

    #[test_log::test(tokio::test)]
    async fn test_save_pdf_check_commands() {
        // 必要コマンド存在チェック (簡易)
        for cmd in ["pdfinfo", "pdftocairo", "pdftohtml", "pdftotext"] {
            if std::process::Command::new(cmd)
                .arg("--help")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_err()
            {
                tracing::warn!("[skip] missing command: {}", cmd);
                return; // skip
            }
        }
        let tp = TestPapers::setup().await.expect("setup papers");
        let sample = &tp.papers[0];
        let local_path = sample.dest_path(&tp.tmp_dir);
        assert!(local_path.exists(), "local sample not found");

        let mut config = ParserConfig::new();
        let t0 = std::time::Instant::now();
        save_pdf(local_path.to_str().unwrap(), &mut config, true, t0)
            .await
            .expect("save_pdf local sample");

        assert!(config.pdf_info.get("pages").is_some(), "pages info missing");
        assert!(config.pdf_figures.len() > 0, "no figures generated");
        assert!(config.sections.len() > 0, "no sections extracted");

        let _ = config.clean_files();
        let _ = tp.cleanup();
    }

    #[test_log::test(tokio::test)]
    async fn test_invalid_pdf_url() {
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        let url = "https://www.semanticscholar.org/reader/204e3073870fae3d05bcbc2f6a8e263d9b72e776";
        let res = save_pdf(url, &mut config, true, time).await;

        match res {
            Ok(_) => assert!(false),
            Err(e) => {
                tracing::info!("{}", e);
                assert!(true);
            }
        }
    }

    #[test_log::test(tokio::test)]
    async fn test_save_pdf_1() {
        let tp = TestPapers::setup().await.expect("setup test papers");
        let paper = tp.get_by_title(BuiltinPaper::AttentionIsAllYouNeed).expect("paper not found");
        let local_path = paper.dest_path(&tp.tmp_dir);
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        save_pdf(local_path.to_str().unwrap(), &mut config, true, time).await.unwrap();

        assert!(Path::new(&config.pdf_path).exists());

        for (_, path) in config.pdf_figures.iter() {
            tracing::info!("path: {}", path);
            assert!(Path::new(path).exists());
        }

        // Check key sections are present (order may vary with new font scoring)
        let section_names: Vec<&str> = config.sections.iter().map(|(_, s)| s.as_str()).collect();
        for expected in &["Introduction", "Background", "Conclusion", "References"] {
            assert!(
                section_names.contains(expected),
                "Expected section '{}' not found in {:?}",
                expected,
                section_names
            );
        }
        // Should have at least the core sections
        assert!(
            config.sections.len() >= 5,
            "Expected at least 5 sections, got {}",
            config.sections.len()
        );

        for (page, section) in config.sections.iter() {
            tracing::info!("page: {}, section: {}", page, section);
        }

        let _ = config.clean_files();
        let _ = tp.cleanup();
    }

    #[test_log::test(tokio::test)]
    async fn test_save_pdf_2() {
        let tp = TestPapers::setup().await.expect("setup papers");
        let paper = tp.get_by_title(BuiltinPaper::AlgorithmOfThoughts).expect("paper not found");
        let local_path = paper.dest_path(&tp.tmp_dir);
        assert!(local_path.exists(), "cached sample missing");
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        save_pdf(local_path.to_str().unwrap(), &mut config, true, time).await.unwrap();

        assert!(Path::new(&config.pdf_path).exists());

        for (_, path) in config.pdf_figures.iter() {
            tracing::info!("path: {}", path);
            assert!(Path::new(path).exists());
        }

        let section_names: Vec<&str> = config.sections.iter().map(|(_, s)| s.as_str()).collect();
        for expected in &["Introduction", "Experiments", "Conclusion", "References"] {
            assert!(
                section_names.contains(expected),
                "Expected section '{}' not found in {:?}",
                expected,
                section_names
            );
        }
        assert!(
            config.sections.len() >= 7,
            "Expected at least 7 sections, got {}",
            config.sections.len()
        );

        for (page, section) in config.sections.iter() {
            tracing::info!("page: {}, section: {}", page, section);
        }

        let _ = config.clean_files();
        let _ = tp.cleanup();
    }

    #[test_log::test(tokio::test)]
    async fn test_save_pdf_3() {
        let tp = TestPapers::setup().await.expect("setup papers");
        let paper =
            tp.get_by_title(BuiltinPaper::UnsupervisedDialoguePolicies).expect("paper not found");
        let local_path = paper.dest_path(&tp.tmp_dir);
        assert!(local_path.exists(), "cached sample missing");
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        save_pdf(local_path.to_str().unwrap(), &mut config, true, time).await.unwrap();

        assert!(Path::new(&config.pdf_path).exists());

        for (_, path) in config.pdf_figures.iter() {
            tracing::info!("path: {}", path);
            assert!(Path::new(path).exists());
        }

        let section_names: Vec<&str> = config.sections.iter().map(|(_, s)| s.as_str()).collect();
        for expected in &[
            "Introduction",
            "Background",
            "Method",
            "Conclusion",
            "References",
        ] {
            assert!(
                section_names.contains(expected),
                "Expected section '{}' not found in {:?}",
                expected,
                section_names
            );
        }
        assert!(
            config.sections.len() >= 7,
            "Expected at least 7 sections, got {}",
            config.sections.len()
        );

        for (page, section) in config.sections.iter() {
            tracing::info!("page: {}, section: {}", page, section);
        }

        let _ = config.clean_files();
        let _ = tp.cleanup();
    }
    #[test_log::test(tokio::test)]
    async fn test_save_pdf_4() {
        let tp = TestPapers::setup().await.expect("setup papers");
        let paper =
            tp.get_by_title(BuiltinPaper::LearningToUseAiForLearning).expect("paper not found");
        let local_path = paper.dest_path(&tp.tmp_dir);
        assert!(local_path.exists(), "cached sample missing");
        let time = std::time::Instant::now();
        let mut config = ParserConfig::new();
        save_pdf(local_path.to_str().unwrap(), &mut config, true, time).await.unwrap();

        assert!(Path::new(&config.pdf_path).exists());

        for (_, path) in config.pdf_figures.iter() {
            tracing::info!("path: {}", path);
            assert!(Path::new(path).exists());
        }

        let section_names: Vec<&str> = config.sections.iter().map(|(_, s)| s.as_str()).collect();
        for expected in &["Introduction", "Related Work", "References"] {
            assert!(
                section_names.contains(expected),
                "Expected section '{}' not found in {:?}",
                expected,
                section_names
            );
        }
        assert!(
            config.sections.len() >= 5,
            "Expected at least 5 sections, got {}",
            config.sections.len()
        );

        for (page, section) in config.sections.iter() {
            tracing::info!("page: {}, section: {}", page, section);
        }

        let _ = config.clean_files();
        let _ = tp.cleanup();
    }
}
