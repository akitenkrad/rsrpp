use super::*;

#[tokio::test]
async fn test_save_pdf() {
    let url = "https://arxiv.org/pdf/1706.03762";
    let path = save_pdf(url).await.unwrap();
    assert!(Path::new(&path).exists());

    if Path::new(&path).exists() {
        std::fs::remove_file(&path).unwrap();
    }
}

#[tokio::test]
async fn test_pdf2html_url() {
    let url = "https://arxiv.org/pdf/1706.03762";
    let res = pdf2html(url).await;
    let html = res.unwrap();
    assert!(html.html().contains("arXiv:1706.03762"));
}

#[tokio::test]
async fn test_pdf2html_file() {
    let url = "https://arxiv.org/pdf/1706.03762";
    let response = request::get(url).await.unwrap();
    let bytes = response.bytes().await.unwrap();
    let path = "/tmp/test.pdf";
    let mut file = File::create(path).unwrap();
    std::io::copy(&mut bytes.as_ref(), &mut file).unwrap();

    let res = pdf2html("/tmp/test.pdf").await;
    let html = res.unwrap();
    assert!(html.html().contains("arXiv:1706.03762"));

    if Path::new(path).exists() {
        std::fs::remove_file(path).unwrap();
    }
}

#[tokio::test]
async fn test_parse_html() {
    let url = "https://arxiv.org/pdf/1706.03762";
    let res = pdf2html(url).await;
    let html = res.unwrap();

    let pages = parse_html(&html).unwrap();
    assert!(pages.len() > 0);
    let text = pages[0].blocks[0].lines[0].get_text();
    assert_eq!(
        text.trim(),
        "Provided proper attribution is provided, Google hereby grants permission to"
    );
}
