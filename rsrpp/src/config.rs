use anyhow::Result;
use rand::Rng;
use std::collections::HashMap;
use std::path::Path;

pub type PageNumber = i16;

/// `ParserConfig` is a configuration structure for parsing PDF documents.
///
/// # Fields
///
/// * `pdf_path` - The file path to the PDF document.
/// * `pdf_text_path` - The file path to the extracted text from the PDF document.
/// * `pdf_figures` - A map of page numbers to file paths of extracted figures from the PDF document.
/// * `pdf_xml_path` - The file path to the extracted XML data from the PDF document.
/// * `sections` - A vector of tuples containing page numbers and section titles.
/// * `pdf_info` - A map containing metadata information about the PDF document.
///
/// # Methods
///
/// * `new` - Creates a new instance of `ParserConfig` with default values.
/// * `pdf_width` - Returns the width of the PDF document as an `i32`.
/// * `pdf_height` - Returns the height of the PDF document as an `i32`.
/// * `clean_files` - Removes the PDF, text, XML, and figure files associated with the `ParserConfig`.
//
#[derive(Debug, Clone, PartialEq)]
pub struct ParserConfig {
    pub pdf_path: String,
    pub pdf_text_path: String,
    pub pdf_figures: HashMap<PageNumber, String>,
    pub pdf_xml_path: String,
    pub sections: Vec<(PageNumber, String)>,
    pub pdf_info: HashMap<String, String>,
    pub use_llm: bool,
}

impl ParserConfig {
    /// Creates a new `ParserConfig` instance with default values.
    ///
    /// This function initializes the following fields:
    /// - `pdf_path`: A randomly generated file path in the `/tmp` directory.
    /// - `pdf_text_path`: The path to the HTML text version of the PDF.
    /// - `pdf_figures`: An empty `HashMap` to store figures extracted from the PDF.
    /// - `pdf_xml_path`: The path to the raw XML version of the PDF.
    /// - `sections`: An empty vector to store sections of the parsed PDF.
    /// - `pdf_info`: An empty `HashMap` to store additional PDF information.
    ///
    /// # Returns
    ///
    /// A new `ParserConfig` instance with the initialized fields.
    pub fn new() -> ParserConfig {
        let mut rng = rand::rng();
        let random_value = rng.random_range(10000..99999);
        let mut pdf_path = String::new();
        pdf_path.push_str("/tmp/pdf_");
        pdf_path.push_str(&random_value.to_string());
        pdf_path.push_str(".pdf");

        let pdf_figures = HashMap::new();
        let pdf_html_path = pdf_path.clone().replace(".pdf", ".text.html");
        let pdf_raw_html_path = pdf_path.clone().replace(".pdf", ".xml");
        let sections = Vec::new();
        ParserConfig {
            pdf_path: pdf_path,
            pdf_text_path: pdf_html_path,
            pdf_figures: pdf_figures,
            pdf_xml_path: pdf_raw_html_path,
            sections: sections,
            pdf_info: HashMap::new(),
            use_llm: false,
        }
    }

    /// Returns the width of the PDF page.
    ///
    /// This function retrieves the width of the PDF page from the `pdf_info` field,
    /// which is a `HashMap` containing additional information about the PDF.
    ///
    /// # Returns
    ///
    /// A `Result<i32>` representing the width of the PDF page.
    ///
    /// # Errors
    ///
    /// Returns an error if the `page_width` key is not found in the `pdf_info`
    /// `HashMap` or if the value cannot be parsed as an `i32`.
    pub fn pdf_width(&self) -> anyhow::Result<i32> {
        self.pdf_info
            .get("page_width")
            .ok_or_else(|| anyhow::anyhow!("PDF width not available - pdfinfo may have failed"))?
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("Invalid page_width value: {}", e))
    }

    /// Returns the height of the PDF page.
    ///
    /// This function retrieves the height of the PDF page from the `pdf_info` field,
    /// which is a `HashMap` containing additional information about the PDF.
    ///
    /// # Returns
    ///
    /// A `Result<i32>` representing the height of the PDF page.
    ///
    /// # Errors
    ///
    /// Returns an error if the `page_height` key is not found in the `pdf_info`
    /// `HashMap` or if the value cannot be parsed as an `i32`.
    pub fn pdf_height(&self) -> anyhow::Result<i32> {
        self.pdf_info
            .get("page_height")
            .ok_or_else(|| anyhow::anyhow!("PDF height not available - pdfinfo may have failed"))?
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("Invalid page_height value: {}", e))
    }

    /// Cleans up the generated files associated with the `ParserConfig` instance.
    ///
    /// This function removes the following files if they exist:
    /// - The PDF file at `pdf_path`.
    /// - The HTML text version of the PDF at `pdf_text_path`.
    /// - The raw XML version of the PDF at `pdf_xml_path`.
    /// - Any files associated with figures stored in the `pdf_figures` `HashMap`.
    ///
    /// # Returns
    ///
    /// A `Result` indicating the success or failure of the file removal operations.
    ///
    /// # Errors
    ///
    /// This function will return an error if any of the file removal operations fail.
    pub fn clean_files(&self) -> Result<()> {
        if Path::new(&self.pdf_path).exists() {
            std::fs::remove_file(&self.pdf_path)?;
        }
        if Path::new(&self.pdf_text_path).exists() {
            std::fs::remove_file(&self.pdf_text_path)?;
        }
        if Path::new(&self.pdf_xml_path).exists() {
            std::fs::remove_file(&self.pdf_xml_path)?;
        }
        for figure in self.pdf_figures.values() {
            if Path::new(figure).exists() {
                std::fs::remove_file(figure)?;
            }
        }
        return Ok(());
    }
}
