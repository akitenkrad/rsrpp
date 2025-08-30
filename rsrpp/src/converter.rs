use crate::config::{PageNumber, ParserConfig};
use anyhow::{Error, Result};
use glob::glob;
use indicatif::ProgressBar;
use quick_xml::events::Event;
use reqwest as request;
use scraper::html;
use std::{
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
            let caps = regex.captures(&value).unwrap();
            config.pdf_info.insert("page_width".to_string(), caps[1].to_string());
            config.pdf_info.insert("page_height".to_string(), caps[2].to_string());
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
                config.pdf_figures.insert(page_number, path.to_str().unwrap().to_string());
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
                if String::from_utf8_lossy(e.as_ref()).to_lowercase() == "abstract"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "introduction"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "background"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "method"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "related work"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "related works"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "experiments"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "conclusion"
                    || String::from_utf8_lossy(e.as_ref()).to_lowercase() == "references"
                {
                    break;
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

    if verbose {
        tracing::info!(
            "Extracted Title Font Size in {:.2}s",
            time.elapsed().as_secs()
        );
    }

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
    let mut page_number = 0;
    let mut start_paper = false;
    let mut is_title = false;
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
                        if cfg!(test) {
                            tracing::info!("Found title: {} pt", font_number);
                        }
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
                    if text.to_lowercase() == "abstract" {
                        start_paper = true;
                    }
                    if !start_paper {
                        continue;
                    }

                    if cfg!(test) {
                        tracing::info!("Found section title: {}", text);
                    }
                    config.sections.push((page_number, text.to_string()));
                    if text.to_lowercase() == "references" {
                        break;
                    }
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

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    if verbose {
        tracing::info!("Converted PDf into XML in {:.2}s", time.elapsed().as_secs());
    }

    return Ok(());
}

pub(crate) fn save_pdf_as_text(
    config: &mut ParserConfig,
    verbose: bool,
    time: std::time::Instant,
) -> Result<()> {
    let html_path = Path::new(config.pdf_text_path.as_str());

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

        assert_eq!(config.sections[0].1, "Abstract".to_string());
        assert_eq!(config.sections[1].1, "Introduction".to_string());
        assert_eq!(config.sections[2].1, "Background".to_string());
        assert_eq!(config.sections[3].1, "Model Architecture".to_string());
        assert_eq!(config.sections[4].1, "Why Self-Attention".to_string());
        assert_eq!(config.sections[5].1, "Training".to_string());
        assert_eq!(config.sections[6].1, "Results".to_string());
        assert_eq!(config.sections[7].1, "Conclusion".to_string());
        assert_eq!(config.sections[8].1, "References".to_string());

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

        assert_eq!(config.sections[0].1, "Abstract".to_string());
        assert_eq!(config.sections[1].1, "Introduction".to_string());
        assert_eq!(config.sections[2].1, "Related Work".to_string());
        assert_eq!(config.sections[3].1, "Algorithm of Thoughts".to_string());
        assert_eq!(config.sections[4].1, "Experiments".to_string());
        assert_eq!(config.sections[5].1, "Discussion".to_string());
        assert_eq!(config.sections[6].1, "Conclusion".to_string());
        assert_eq!(config.sections[7].1, "Limitations".to_string());
        assert_eq!(config.sections[8].1, "Acknowledgments".to_string());
        assert_eq!(config.sections[9].1, "Impact Statement".to_string());
        assert_eq!(config.sections[10].1, "References".to_string());

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

        assert_eq!(config.sections[0].1, "Abstract".to_string());
        assert_eq!(config.sections[1].1, "Introduction".to_string());
        assert_eq!(config.sections[2].1, "Background".to_string());
        assert_eq!(config.sections[3].1, "Method".to_string());
        assert_eq!(config.sections[4].1, "Experimental Settings".to_string());
        assert_eq!(config.sections[5].1, "Results and Analysis".to_string());
        assert_eq!(config.sections[6].1, "Conclusion".to_string());
        assert_eq!(config.sections[7].1, "Limitations and Risks".to_string());
        assert_eq!(config.sections[8].1, "Broader Impact".to_string());
        assert_eq!(config.sections[9].1, "References".to_string());

        for (page, section) in config.sections.iter() {
            tracing::info!("page: {}, section: {}", page, section);
        }

        let _ = config.clean_files();
        let _ = tp.cleanup();
    }
}
