use anyhow::Result;
use rand::RngExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::models::Reference;
use serde::{Deserialize, Serialize};

pub type PageNumber = i16;

/// Counts the alphanumeric characters in `text`.
///
/// Coverage is compared on alphanumerics only so that whitespace and layout
/// differences between poppler's reading order and the block structure do not
/// register as lost text.
pub(crate) fn normalized_len(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// Why a piece of text was discarded during parsing.
///
/// Every variant corresponds to one geometric or heuristic filter in the pipeline.
/// Discarded text is recorded in [`ParserConfig::dropped_texts`] rather than thrown
/// away silently, so callers can audit what a parse left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DropReason {
    /// The line sits inside a region detected as a table.
    TableRegion,
    /// The block does not overlap the page's dominant text area at all.
    OutsideTextArea,
    /// The block is too narrow and too short to be body text.
    NarrowBlock,
}

impl std::fmt::Display for DropReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DropReason::TableRegion => "table region",
            DropReason::OutsideTextArea => "outside text area",
            DropReason::NarrowBlock => "narrow block",
        };
        f.write_str(s)
    }
}

/// A fragment of text that a filter removed from the parsed output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedText {
    pub page: PageNumber,
    pub reason: DropReason,
    pub text: String,
}

/// The temporary directory one parse works in, removed whole when the last handle
/// to it is dropped.
///
/// The directory is what gets tracked, not the individual files. Poppler decides how
/// many files a parse produces and when: `pdftohtml -c` writes one image per embedded
/// figure, long after `save_pdf_as_figures` has globbed the directory for page images,
/// so any list of names collected during a parse is already out of date. One paper in
/// the corpus leaves 1,079,890 files behind — more than a `rm` glob can even accept as
/// arguments. Removing the directory itself cannot miss anything poppler wrote into it.
///
/// [`ParserConfig`] holds this behind an `Arc` so that cloning a config shares the
/// directory rather than giving two owners the right to delete it; the removal happens
/// once, when the last clone goes away.
#[derive(Debug)]
pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(path: PathBuf) -> Self {
        // Best effort: `ParserConfig::new` cannot fail, so a directory that could not be
        // created here is reported by `ensure_exists` when the parse first needs it.
        if let Err(e) = std::fs::create_dir_all(&path) {
            tracing::warn!(
                "Failed to create temporary directory {}: {}",
                path.display(),
                e
            );
        }
        TempDir { path }
    }

    /// The directory itself, so the output watchdog knows what to measure.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Creates the directory if it is missing, so a parse never writes into a directory
    /// that `new` failed to create or that a previous `clean_files` removed.
    pub(crate) fn ensure_exists(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)?;
        return Ok(());
    }

    /// Removes the directory and everything poppler put in it.
    ///
    /// Idempotent: a directory that is already gone is the intended end state, not an
    /// error. Callers may clean up explicitly and still have [`Drop`] run afterwards.
    pub(crate) fn remove(&self) -> Result<()> {
        match std::fs::remove_dir_all(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Errors cannot be propagated out of `drop`, and a failure here must not mask the
        // error that is already unwinding, so it is logged and the parse result stands.
        if let Err(e) = self.remove() {
            tracing::warn!(
                "Failed to remove temporary directory {}: {}",
                self.path.display(),
                e
            );
        }
    }
}

impl PartialEq for TempDir {
    /// Two guards are equal when they point at the same directory. The directory path is
    /// unique per guard, so this only ever holds between clones of one config.
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

/// How long one poppler process may run before a parse gives up on it.
///
/// Sized from the slowest healthy document in the reference corpus: `pdftohtml` on a
/// 357-page scan takes 238s. Fifteen minutes leaves that a factor of ~3.8, which also
/// covers the 1,008-page scan in the same corpus, while still bounding a hang.
pub const DEFAULT_POPPLER_TIMEOUT: Duration = Duration::from_secs(900);

/// How many files poppler may write per page of the document before the parse treats
/// it as a runaway.
///
/// The gap this sits in is three orders of magnitude wide. A healthy 357-page scan
/// leaves 1,431 files in the working directory across all four poppler calls — 4.0 per
/// page. The one pathological document in the corpus (17 pages, a figure filled with 99
/// tiling patterns whose cells are 1pt and 4pt, so `pdftohtml` writes one tiny PNG per
/// tile) produced 1,079,890 files — 63,500 per page. 200 leaves the healthy end a factor
/// of 50 and still stops the runaway inside its first second.
pub const DEFAULT_POPPLER_MAX_FILES_PER_PAGE: usize = 200;

/// How many bytes poppler may write per page of the document.
///
/// A second, independent cap: the file count catches output that is unbounded in
/// *number*, this catches output unbounded in *size*. The same 357-page scan occupies
/// 168 MB (470 KB per page), so 10 MB per page leaves a factor of ~21.
pub const DEFAULT_POPPLER_MAX_BYTES_PER_PAGE: u64 = 10 * 1024 * 1024;

/// The smallest file budget any document gets, however few pages it has.
///
/// Without a floor a short document would be held to a budget of a handful of files,
/// and per-page limits are a poor description of a 1-page PDF: page count is only a
/// proxy for how much work is legitimately in the document.
pub const MIN_POPPLER_FILE_BUDGET: usize = 2_000;

/// The smallest byte budget any document gets. See [`MIN_POPPLER_FILE_BUDGET`].
pub const MIN_POPPLER_BYTE_BUDGET: u64 = 200 * 1024 * 1024;

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
/// * `use_llm` - Whether to use LLM for enhanced math extraction.
/// * `math_texts` - A map of (page_number, block_index) to math-marked text.
/// * `extract_references` - Whether to extract structured references from the paper.
/// * `references` - Extracted bibliographic references.
/// * `dropped_texts` - Text removed by the parsing filters, with the reason for each drop.
/// * `source_char_count` - Normalized character count of the source text, the denominator of `coverage`.
///
/// # Methods
///
/// * `new` - Creates a new instance of `ParserConfig` with default values.
/// * `pdf_width` - Returns the width of the PDF document as an `i32`.
/// * `pdf_height` - Returns the height of the PDF document as an `i32`.
/// * `clean_files` - Removes the temporary directory holding every file this parse produced.
/// * `reset_document_state` - Clears everything derived from a previously parsed document.
/// * `coverage` - Fraction of the source text that survived the parsing filters.
/// * `dropped_char_count` - Normalized character count of everything that was discarded.
//
/// Construct with [`ParserConfig::new`] and adjust the settings you need.
///
/// Marked `#[non_exhaustive]` because this type doubles as settings and as the place
/// results are reported: the result fields grow whenever the parser learns to report
/// something new, and each addition would otherwise break every downstream struct
/// literal. `new()` has always been the only sensible way to build one.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ParserConfig {
    pub pdf_path: String,
    pub pdf_text_path: String,
    pub pdf_figures: HashMap<PageNumber, String>,
    pub pdf_xml_path: String,
    pub sections: Vec<(PageNumber, String)>,
    pub pdf_info: HashMap<String, String>,
    pub use_llm: bool,
    pub math_texts: HashMap<(PageNumber, usize), String>,
    pub extract_references: bool,
    pub references: Vec<Reference>,
    /// Text removed by the geometric/heuristic filters, with the reason for each drop.
    /// Populated during parsing; never dropped silently.
    pub dropped_texts: Vec<DroppedText>,
    /// Number of normalized characters poppler produced for this PDF, i.e. the
    /// denominator of [`ParserConfig::coverage`]. `0` until parsing has run.
    pub source_char_count: usize,
    /// How long a *single* poppler invocation may run before the parse kills it and
    /// fails the document. Four processes run per parse (`pdfinfo`, `pdftocairo`,
    /// `pdftohtml`, `pdftotext`), so the budget for a whole document is larger.
    ///
    /// Defaults to [`DEFAULT_POPPLER_TIMEOUT`]. Raise it for corpora of very long
    /// scanned documents; those are slow but well behaved, and cutting them off loses
    /// papers that would have parsed.
    pub poppler_timeout: Duration,
    /// How many files poppler may leave in the working directory, per page of the
    /// document. See [`DEFAULT_POPPLER_MAX_FILES_PER_PAGE`].
    ///
    /// This is the limit that catches a runaway; the timeout alone cannot, because
    /// "slow because it is long" and "writing without bound" look the same from the
    /// outside until you divide by the page count.
    pub poppler_max_files_per_page: usize,
    /// How many bytes poppler may leave in the working directory, per page of the
    /// document. See [`DEFAULT_POPPLER_MAX_BYTES_PER_PAGE`].
    pub poppler_max_bytes_per_page: u64,
    /// The temporary directory every path above points into. Removed whole when the
    /// last clone of this config is dropped, or when [`ParserConfig::clean_files`] is
    /// called; see [`TempDir`].
    pub(crate) temp_dir: Arc<TempDir>,
}

impl ParserConfig {
    /// Creates a new `ParserConfig` instance with default values.
    ///
    /// This function initializes the following fields:
    /// - `temp_dir`: A directory of its own under the system temp directory, holding
    ///   every file this parse produces and removed when the config is dropped.
    /// - `pdf_path`: A randomly generated file path inside that directory.
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
        // Working directories are removed on drop, which no `SIGKILL` lets happen. The
        // leftovers of earlier runs are collected here, at most once per process, so that
        // every caller of the crate gets the recovery and not only the CLI. It cannot
        // fail the parse; see `crate::tempdir_sweep`.
        crate::tempdir_sweep::sweep_abandoned_temp_dirs();

        // Unique by construction, not by luck. Two configs alive at once must never
        // share a directory: they delete it in `clean_files` and on drop, so a collision
        // makes one parse quietly remove the other's working files. A random number alone
        // could repeat, which is why the process id and a counter come first; the random
        // part only keeps paths unpredictable across runs.
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let mut rng = rand::rng();
        let stem = format!(
            "pdf_{}_{}_{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed),
            rng.random_range(10000..99999)
        );
        // Everything poppler writes derives its name from `pdf_path`, so putting the PDF
        // inside a directory of its own is enough to catch every by-product, including
        // the ones nothing in this crate ever learns the name of.
        let temp_dir = Arc::new(TempDir::new(std::env::temp_dir().join(&stem)));
        let pdf_path = format!("{}/{}.pdf", temp_dir.path.display(), stem);

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
            use_llm: true,
            math_texts: HashMap::new(),
            extract_references: false,
            references: Vec::new(),
            dropped_texts: Vec::new(),
            source_char_count: 0,
            poppler_timeout: DEFAULT_POPPLER_TIMEOUT,
            poppler_max_files_per_page: DEFAULT_POPPLER_MAX_FILES_PER_PAGE,
            poppler_max_bytes_per_page: DEFAULT_POPPLER_MAX_BYTES_PER_PAGE,
            temp_dir: temp_dir,
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

    /// Clears everything a previous `parse` derived from a document, keeping the
    /// settings that describe *how* to parse.
    ///
    /// `ParserConfig` doubles as settings and as the place results are reported, so a
    /// config reused for a second document would otherwise mix the two documents:
    /// `sections` and `dropped_texts` are appended to rather than replaced, and
    /// `pdf_figures` keeps entries for pages the shorter document does not have.
    ///
    /// Settings (`pdf_path`, `pdf_text_path`, `pdf_xml_path`, `use_llm`,
    /// `extract_references`) are deliberately left alone.
    pub fn reset_document_state(&mut self) {
        self.pdf_figures.clear();
        self.pdf_info.clear();
        self.sections.clear();
        self.math_texts.clear();
        self.references.clear();
        self.dropped_texts.clear();
        self.source_char_count = 0;
    }

    /// Records a fragment of text that a filter removed.
    ///
    /// Blank fragments are ignored — they carry no content and would only add noise.
    pub(crate) fn record_drop(&mut self, page: PageNumber, reason: DropReason, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        tracing::debug!("Dropped from page {} ({}): {}", page, reason, text.trim());
        self.dropped_texts.push(DroppedText {
            page,
            reason,
            text: text.to_string(),
        });
    }

    /// Normalized character count of everything the filters discarded.
    pub fn dropped_char_count(&self) -> usize {
        self.dropped_texts.iter().map(|d| normalized_len(&d.text)).sum()
    }

    /// Fraction of the source text that survived into the parsed pages, in `0.0..=1.0`.
    ///
    /// Returns `None` before parsing has run (when `source_char_count` is still 0).
    /// This is a *lower bound* on fidelity: it counts characters, so it cannot tell
    /// reordered text from correct text.
    pub fn coverage(&self) -> Option<f32> {
        if self.source_char_count == 0 {
            return None;
        }
        let dropped = self.dropped_char_count().min(self.source_char_count);
        Some(1.0 - dropped as f32 / self.source_char_count as f32)
    }

    /// Removes the temporary directory this config works in, and with it the PDF copy,
    /// the extracted text and XML, the page images and every other file poppler wrote.
    ///
    /// Calling this is optional: the same removal happens when the last clone of the
    /// config is dropped, which is what covers the `?` and panic paths. It exists for
    /// callers that want the space back before the config itself goes away.
    ///
    /// # Returns
    ///
    /// A `Result` indicating the success or failure of the removal.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory exists but cannot be removed. Calling it on a
    /// directory that is already gone succeeds — it is idempotent.
    pub fn clean_files(&self) -> Result<()> {
        return self.temp_dir.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_new_creates_a_directory_of_its_own() {
        let config = ParserConfig::new();
        let dir = Path::new(&config.pdf_path).parent().unwrap().to_path_buf();
        assert!(dir.is_dir(), "the parse has nowhere to write");
        // Every path the parser hands to poppler must sit inside that directory,
        // otherwise removing it would not be enough.
        assert_eq!(Path::new(&config.pdf_text_path).parent().unwrap(), dir);
        assert_eq!(Path::new(&config.pdf_xml_path).parent().unwrap(), dir);
    }

    #[test]
    fn test_clean_files_removes_files_nothing_tracked() {
        // poppler writes one image per embedded figure under names this crate never
        // learns, so the removal has to be by directory, not by list.
        let config = ParserConfig::new();
        let dir = Path::new(&config.pdf_path).parent().unwrap().to_path_buf();
        std::fs::write(dir.join("untracked-1_1.png"), b"x").unwrap();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/untracked-2_7.png"), b"x").unwrap();

        config.clean_files().unwrap();

        assert!(!dir.exists(), "untracked by-products survived the cleanup");
    }

    #[test]
    fn test_clean_files_is_idempotent() {
        // Tests call it explicitly and `Drop` calls it again; the second call must not
        // turn an already-clean state into an error.
        let config = ParserConfig::new();
        config.clean_files().unwrap();
        config.clean_files().expect("cleaning an already-clean config is not a failure");
    }

    #[test]
    fn test_dropping_the_config_removes_the_directory() {
        // This is what covers the `?` and panic paths, where nothing calls clean_files.
        let config = ParserConfig::new();
        let dir = Path::new(&config.pdf_path).parent().unwrap().to_path_buf();
        std::fs::write(dir.join("leftover.jpg"), b"x").unwrap();

        drop(config);

        assert!(
            !dir.exists(),
            "the directory outlived the config that owns it"
        );
    }

    #[test]
    fn test_a_clone_does_not_delete_the_directory_out_from_under_the_original() {
        // `ParserConfig` is `Clone`, so the directory is shared, not owned twice.
        let config = ParserConfig::new();
        let dir = Path::new(&config.pdf_path).parent().unwrap().to_path_buf();

        drop(config.clone());

        assert!(
            dir.is_dir(),
            "a dropped clone took the live config's directory"
        );

        drop(config);

        assert!(!dir.exists(), "the last handle must still clean up");
    }

    #[test]
    fn test_two_configs_do_not_share_a_directory() {
        let a = ParserConfig::new();
        let b = ParserConfig::new();
        let dir_a = Path::new(&a.pdf_path).parent().unwrap().to_path_buf();
        let dir_b = Path::new(&b.pdf_path).parent().unwrap().to_path_buf();
        assert_ne!(dir_a, dir_b);

        drop(a);

        assert!(
            dir_b.is_dir(),
            "one parse cleaning up removed another's files"
        );
    }

    #[test]
    fn test_coverage_is_none_before_parsing() {
        assert!(ParserConfig::new().coverage().is_none());
    }

    #[test]
    fn test_coverage_counts_only_alphanumerics() {
        let mut config = ParserConfig::new();
        config.source_char_count = 100;
        // 8 alphanumerics; the punctuation and spaces do not count.
        config.record_drop(1, DropReason::TableRegion, "a b, c d! ef gh");
        assert_eq!(config.dropped_char_count(), 8);
        assert_eq!(config.coverage(), Some(0.92));
    }

    #[test]
    fn test_blank_drops_are_not_recorded() {
        let mut config = ParserConfig::new();
        config.record_drop(1, DropReason::NarrowBlock, "   \n ");
        assert!(config.dropped_texts.is_empty());
    }

    #[test]
    fn test_reset_document_state_clears_everything_derived_from_a_document() {
        let mut config = ParserConfig::new();
        config.source_char_count = 500;
        config.record_drop(1, DropReason::TableRegion, "55%");
        config.sections.push((1, "Abstract".to_string()));
        config.pdf_figures.insert(1, "/tmp/page-1.png".to_string());
        config.pdf_info.insert("page_width".to_string(), "595".to_string());
        config.math_texts.insert((1, 0), "<math>x</math>".to_string());

        config.reset_document_state();

        assert!(config.dropped_texts.is_empty());
        assert_eq!(config.source_char_count, 0);
        assert!(config.sections.is_empty());
        assert!(config.pdf_figures.is_empty());
        assert!(config.pdf_info.is_empty());
        assert!(config.math_texts.is_empty());
        assert!(config.references.is_empty());
        assert!(
            config.coverage().is_none(),
            "coverage has no meaning before the next parse"
        );
    }

    #[test]
    fn test_reset_document_state_keeps_settings() {
        // Settings describe how to parse, not what was parsed, so they must survive.
        let mut config = ParserConfig::new();
        config.use_llm = false;
        config.extract_references = true;
        let pdf_path = config.pdf_path.clone();
        let text_path = config.pdf_text_path.clone();
        let xml_path = config.pdf_xml_path.clone();

        config.reset_document_state();

        assert!(!config.use_llm);
        assert!(config.extract_references);
        assert_eq!(config.pdf_path, pdf_path);
        assert_eq!(config.pdf_text_path, text_path);
        assert_eq!(config.pdf_xml_path, xml_path);
    }

    #[test]
    fn test_coverage_is_not_polluted_by_a_previous_document() {
        let mut config = ParserConfig::new();
        // First document: 50 of 100 characters discarded.
        config.source_char_count = 100;
        config.record_drop(1, DropReason::TableRegion, &"a".repeat(50));
        assert_eq!(config.coverage(), Some(0.5));

        // Second document, same config: nothing discarded, so coverage is 100%.
        config.reset_document_state();
        config.source_char_count = 100;
        assert_eq!(config.coverage(), Some(1.0));
    }

    #[test]
    fn test_coverage_is_clamped_when_drops_exceed_source() {
        // Defensive: a caller could set an implausible source count. Coverage must
        // stay in 0.0..=1.0 rather than going negative.
        let mut config = ParserConfig::new();
        config.source_char_count = 2;
        config.record_drop(1, DropReason::OutsideTextArea, "abcdef");
        assert_eq!(config.coverage(), Some(0.0));
    }
}
