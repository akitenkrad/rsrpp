use super::*;

#[tokio::test]
async fn test_invalid_pdf_url() {
    let time = std::time::Instant::now();
    let mut config = ParserConfig::new();
    let url = "https://www.semanticscholar.org/reader/204e3073870fae3d05bcbc2f6a8e263d9b72e776";
    // let url = "https://arxiv.org/pdf/2308.10379";
    let res = save_pdf(url, &mut config, true, time).await;

    match res {
        Ok(_) => assert!(false),
        Err(e) => {
            tracing::info!("{}", e);
            assert!(true);
        }
    }
}

#[tokio::test]
async fn test_save_pdf_1() {
    let time = std::time::Instant::now();
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/1706.03762";
    // let url = "https://arxiv.org/pdf/2308.10379";
    save_pdf(url, &mut config, true, time).await.unwrap();

    assert!(Path::new(&config.pdf_path).exists());

    for (_, path) in config.pdf_figures.iter() {
        tracing::info!("path: {}", path);
        assert!(Path::new(path).exists());
    }

    assert_eq!(config.sections[0], (1, "Abstract".to_string()));
    assert_eq!(config.sections[1], (2, "Introduction".to_string()));
    assert_eq!(config.sections[2], (2, "Background".to_string()));
    assert_eq!(config.sections[3], (2, "Model Architecture".to_string()));
    assert_eq!(config.sections[4], (6, "Why Self-Attention".to_string()));
    assert_eq!(config.sections[5], (7, "Training".to_string()));
    assert_eq!(config.sections[6], (8, "Results".to_string()));
    assert_eq!(config.sections[7], (10, "Conclusion".to_string()));
    assert_eq!(config.sections[8], (10, "References".to_string()));

    for (page, section) in config.sections.iter() {
        tracing::info!("page: {}, section: {}", page, section);
    }

    let _ = config.clean_files();
}

#[tokio::test]
async fn test_adjust_columns() {
    let time = std::time::Instant::now();
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/2411.19655";

    let html = pdf2html(url, &mut config, true, time).await.unwrap();

    // parse html into pages
    let mut pages = parse_html2pages(&mut config, html).unwrap();

    // compare text area and blocks
    parse_extract_textarea(&mut config, &mut pages).unwrap();

    // adjust columns
    adjst_columns(&mut pages, &mut config);

    tracing::info!("{}", &pages[0].number_of_columns);
    let sections = Section::from_pages(&pages);
    for section in sections.iter() {
        tracing::info!("{}: {}", section.title, section.get_text());
    }

    assert_eq!(pages[0].number_of_columns, 2);
}

#[tokio::test]
async fn test_save_pdf_2() {
    let time = std::time::Instant::now();
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/2308.10379";
    save_pdf(url, &mut config, true, time).await.unwrap();

    assert!(Path::new(&config.pdf_path).exists());

    for (_, path) in config.pdf_figures.iter() {
        tracing::info!("path: {}", path);
        assert!(Path::new(path).exists());
    }

    assert_eq!(config.sections[0], (1, "Abstract".to_string()));
    assert_eq!(config.sections[1], (1, "Introduction".to_string()));
    assert_eq!(config.sections[2], (3, "Related Work".to_string()));
    assert_eq!(config.sections[3], (4, "Algorithm of Thoughts".to_string()));
    assert_eq!(config.sections[4], (5, "Experiments".to_string()));
    assert_eq!(config.sections[5], (8, "Discussion".to_string()));
    assert_eq!(config.sections[6], (9, "Conclusion".to_string()));
    assert_eq!(config.sections[7], (9, "Limitations".to_string()));
    assert_eq!(config.sections[8], (10, "Acknowledgments".to_string()));
    assert_eq!(config.sections[9], (10, "Impact Statement".to_string()));
    assert_eq!(config.sections[10], (10, "References".to_string()));

    for (page, section) in config.sections.iter() {
        tracing::info!("page: {}, section: {}", page, section);
    }

    let _ = config.clean_files();
}

#[tokio::test]
async fn test_pdf2html_url() {
    let time = std::time::Instant::now();
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/1706.03762";
    let res = pdf2html(url, &mut config, true, time).await;
    let html = res.unwrap();
    assert!(html.html().contains("arXiv:1706.03762"));
    let _ = config.clean_files();
}

#[tokio::test]
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

#[tokio::test]
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
            let block_coord = Coordinate::from_object(block.x, block.y, block.width, block.height);
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

#[tokio::test]
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
            let block_coord = Coordinate::from_object(block.x, block.y, block.width, block.height);
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

#[tokio::test]
async fn test_parse_3() {
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/2410.24080";
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
            let block_coord = Coordinate::from_object(block.x, block.y, block.width, block.height);
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
async fn test_pdf_to_json_1() {
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/1706.03762";
    let pages = parse(url, &mut config, true).await.unwrap();
    let sections = Section::from_pages(&pages);

    for section in sections.iter() {
        assert!(section.title.len() > 0);
        assert!(section.contents.len() > 0);
        tracing::info!("{}: {}", section.title, section.get_text());
    }

    let json = serde_json::to_string(&sections).unwrap();
    tracing::info!("{}", json);
    assert!(json.len() > 0);

    let json = pages2json(&pages);
    tracing::info!("{}", json);
    assert!(json.len() > 0);
}

#[tokio::test]
async fn test_pdf_to_json_2() {
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/2308.10379";
    let pages = parse(url, &mut config, true).await.unwrap();
    let sections = Section::from_pages(&pages);

    for section in sections.iter() {
        assert!(section.title.len() > 0);
        assert!(section.contents.len() > 0);
        tracing::info!("{}: {}", section.title, section.get_text());
    }

    let json = serde_json::to_string(&sections).unwrap();
    tracing::info!("{}", json);
    assert!(json.len() > 0);

    let json = pages2json(&pages);
    tracing::info!("{}", json);
    assert!(json.len() > 0);
}

#[tokio::test]
async fn test_pdf_to_json_3() {
    let mut config = ParserConfig::new();
    let url = "https://arxiv.org/pdf/2410.24080";
    let pages = parse(url, &mut config, true).await.unwrap();
    let sections = Section::from_pages(&pages);

    for section in sections.iter() {
        assert!(section.title.len() > 0);
        assert!(section.contents.len() > 0);
        tracing::info!("{}: {}", section.title, section.get_text());
    }

    let json = serde_json::to_string(&sections).unwrap();
    tracing::info!("{}", json);
    assert!(json.len() > 0);

    let json = pages2json(&pages);
    tracing::info!("{}", json);
    assert!(json.len() > 0);
}
