use crate::config::{PageNumber, ParserConfig};
use anyhow::{Error, Result};
use glob::glob;
use indicatif::ProgressBar;
use quick_xml::events::Event;
use reqwest as request;
use scraper::html;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::Path,
    process::{Command, Stdio},
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

    // ── Step 1: Parse <fontspec> elements ──
    let xml_text = std::fs::read_to_string(xml_path)?;

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
                if e.name().as_ref() == b"fontspec" {
                    let mut id = 0i32;
                    let mut size = 0.0f32;
                    let mut family = String::new();
                    for attr in e.attributes() {
                        let attr = attr?;
                        match attr.key.as_ref() {
                            b"id" => {
                                id = String::from_utf8_lossy(attr.value.as_ref())
                                    .parse::<i32>()
                                    .unwrap_or(0);
                            }
                            b"size" => {
                                size = String::from_utf8_lossy(attr.value.as_ref())
                                    .parse::<f32>()
                                    .unwrap_or(0.0);
                            }
                            b"family" => {
                                family = String::from_utf8_lossy(attr.value.as_ref()).to_string();
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
                if e.name().as_ref() == b"text" {
                    current_font_id = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.as_ref() == b"font")
                        .map(|a| {
                            String::from_utf8_lossy(a.value.as_ref()).parse::<i32>().unwrap_or(0)
                        })
                        .unwrap_or(0);
                    current_text.clear();
                }
            }
            Ok(Event::Text(e)) => {
                current_text.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"text" {
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
    let mut pending_sections: Vec<(PageNumber, String)> = Vec::new();
    let regex_is_number = regex::Regex::new(r"^\d+$").unwrap();
    let regex_trim_number = regex::Regex::new(r"^\d+\.?\s*").unwrap();
    let mut reader = quick_xml::Reader::from_str(&xml_text);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"page" {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"number" {
                            page_number = String::from_utf8_lossy(attr.value.as_ref())
                                .parse::<PageNumber>()
                                .unwrap_or(0);
                        }
                    }
                } else if e.name().as_ref() == b"text" {
                    let font_number = e
                        .attributes()
                        .filter_map(|attr| attr.ok())
                        .find(|attr| attr.key.as_ref() == b"font")
                        .map(|attr| {
                            String::from_utf8_lossy(attr.value.as_ref()).parse::<i32>().unwrap_or(0)
                        })
                        .unwrap_or(0);

                    probably_title = title_font_set.contains(&font_number);
                    continue;
                }
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                if regex_is_number.is_match(&text) {
                    continue;
                }
                let text = regex_trim_number.replace(&text, "").to_string().trim().to_string();

                if text.to_lowercase().trim() == "abstract" {
                    start_paper = true;
                    // Record current pending length so we know where "abstract" appeared
                    // relative to title-font entries. If "abstract" itself is title-font,
                    // it will be pushed next (at this index). If not, the next title-font
                    // text will land at this index.
                    if start_paper_at.is_none() {
                        start_paper_at = Some(pending_sections.len());
                    }
                }

                if probably_title {
                    if cfg!(test) {
                        tracing::info!("Found section title (p{}): {}", page_number, text);
                    }
                    pending_sections.push((page_number, text.to_string()));
                }
            }
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                break;
            }
            _ => {}
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
            config.sections.push(section);
        }
    } else if !pending_sections.is_empty() {
        // Fallback: no "Abstract" heading (e.g. Nature format)
        // Start from the first anchor-word match
        let first_anchor_idx = pending_sections.iter().position(|(_, text)| {
            let t = text.to_lowercase();
            let s = regex_trim_number.replace(&t, "").trim().to_string();
            anchor_words.iter().any(|&aw| aw == t.as_str() || aw == s.as_str())
        });

        if let Some(idx) = first_anchor_idx {
            // If the first anchor section is beyond page 1, infer an Abstract on page 1
            let first_page = pending_sections[idx].0;
            if first_page > 1 {
                config.sections.push((1, "Abstract".to_string()));
            }
            for section in pending_sections.into_iter().skip(idx) {
                config.sections.push(section);
            }
        }
    }

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    if verbose {
        tracing::info!("Converted PDF into XML in {:.2}s", time.elapsed().as_secs());
    }

    return Ok(());
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
