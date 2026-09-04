**English** | [日本語](releases.ja.md)

# Release history

<details open>
<summary>2.0.0</summary>

**Breaking changes.** Two of them, both about behaviour rather than signatures.

- **Temporary files now live and die with the config.** A parse works in a directory of
  its own and removes it when the last handle to the `ParserConfig` goes away — on the
  normal path, on an early return, and while a panic unwinds. Previously nothing was
  removed at all: `clean_files()` existed but only the tests called it, and it could not
  have finished the job anyway, since `pdftohtml -c` keeps writing images after the
  figure paths have been collected. **A path copied out of `config.pdf_figures` no
  longer outlives the config**; copy the file out while the config is alive.
- **The CLI no longer exits by panicking.** Every failure used to be an `unwrap`, so a
  broken PDF, an unwritable output path and a document poppler cannot finish all ended
  the same way: exit code 101 and a backtrace. The exit code now says which: `0` written,
  `1` input or environment, `2` command line, `3` poppler stopped at a limit. Scripts
  that tested for 101 need updating.

**Poppler is now held to limits.** Some PDFs make it write without bound — a figure
painted with tiling patterns one and four points wide has `pdftohtml` emit a PNG per
tile, which measured 1,079,890 files and 7.4 GB from a single parse that never returned.
Every poppler call is watched and killed if it passes either a time limit or an output
limit; both are needed, because a 1,000-page scan is slow for an honest reason and has
to be told apart from a document that will never finish. Budgets are per page, with a
floor for short documents: `DEFAULT_POPPLER_TIMEOUT` (900s),
`DEFAULT_POPPLER_MAX_FILES_PER_PAGE` (200), `DEFAULT_POPPLER_MAX_BYTES_PER_PAGE`
(10 MiB), all settable on `ParserConfig`. A breach comes back as
`converter::PopplerLimitError`, so callers can recognise it by type rather than by
matching on the message.

**Abandoned working directories are reclaimed.** `SIGKILL` runs no destructor, so a
killed run leaves its directory behind and poppler may outlive its parent and keep
writing into it. Each process sweeps once, on its first `ParserConfig::new()`, and takes
only directories whose owning process is provably gone, so parses running side by side
never take each other's files.

**Section detection, measured over 2,812 papers.** The changes that went in over the
preceding days — the placement pass that judges a heading candidate by where it sits, the
caption and table-region rules, per-page column detection — were verified against the
whole corpus rather than a sample. No body text is lost: with `--keep-dropped` the output
is byte-for-byte what poppler read, and across the corpus the net change is +4,830
characters. Sub-headings set at a different indent from their ALL CAPS parents are folded
into the parent section; the text is preserved, the structure is coarser.

**Documentation.** The README is now a front page, with the detail in `docs/` in English
and Japanese. The library example there had been importing from `rsrpp::parser::structs`,
which does not exist.

</details>

<details>
<summary>1.0.25</summary>

- LLM-enhanced processing is now enabled by default (`ParserConfig::new()` sets `use_llm: true`)
  - If `OPENAI_API_KEY` is not set, LLM is automatically disabled at runtime
  - Use `--no-llm` (CLI) or `config.use_llm = false` (library) to explicitly disable
- Fixed LLM section validation discarding sections from pages the LLM hadn't examined
  - `merge_sections()` now uses page-range-aware logic: only validates sections within the LLM-examined page range
  - Sections outside the LLM page range are preserved from font-based detection

</details>

<details>
<summary>1.0.24</summary>

- Fixed body text loss in Nature-format and non-standard papers:
  - Added section detection fallback for papers without "Abstract" heading (e.g., Nature format) using anchor-word matching
  - Added text area degenerate detection to prevent filtering out all blocks when computed text area is too small
  - Capped table detection regions at 50% of page area to reject false positives from chart gridlines and figure borders
  - Exempted known section titles from table-region filtering
- Improved math extraction accuracy:
  - Fixed critical bug where LLM-extracted math text was discarded; added trigram-based block alignment
  - Reduced false positives: dates (`2019/2020`), statistics (`n = 50 participants`), section references
  - Added detection for multi-char math functions (`sin`, `cos`, `log`), ASCII exponents/subscripts (`x^2`, `x_i`), letter fractions (`a/b`), norm notation (`||w||`)
  - Unified math output to LaTeX format inside `<math>` tags (Unicode symbols converted to LaTeX commands)
  - Added context-based validation for structure-only pattern matches
  - Added 25 new tests including comprehensive regression suite

</details>

<details>
<summary>1.0.23</summary>

- Updated crate documentation and version.

</details>

<details>
<summary>1.0.22</summary>

- Added text cleaning and math markup support:
  - New `cleaner` module for caption detection (Figure, Table, Algorithm, etc.)
  - New `llm` module for math expression detection and `<math>...</math>` markup
  - `Section` now has `math_contents` and `captions` fields
  - New `Section::from_pages_with_math()` method for math-marked output
- New CLI options:
  - `--include-captions`: Include captions in main content field
  - `--no-math-markup`: Disable math detection and tagging
  - `--no-llm`: Disable LLM-enhanced processing
- New environment variable `OPENAI_API_MODEL` to specify LLM model (default: gpt-5.2)

</details>

<details>
<summary>1.0.21</summary>

- Fixed panic-causing unwrap() calls with proper error handling.

</details>

<details>
<summary>1.0.20</summary>

- Fixed Poppler 25.12.0 compatibility on macOS.

</details>

<details>
<summary>1.0.19</summary>

- Refactored `fix_suffix_hyphens` to support 31 compound word suffixes:
  - `-based`, `-driven`, `-oriented`, `-aware`, `-agnostic`, `-independent`, `-dependent`, `-first`, `-native`, `-centric`, `-intensive`, `-bound`, `-safe`, `-free`, `-proof`, `-efficient`, `-optimized`, `-enabled`, `-powered`, `-ready`, `-capable`, `-compatible`, `-compliant`, `-level`, `-scale`, `-wide`, `-specific`, `-friendly`, `-facing`, `-like`, `-style`
- Added unit tests for suffix hyphenation functionality.

</details>

<details>
<summary>1.0.18</summary>

- updated how to extract section titles from PDF.

</details>

<details>
<summary>1.0.17</summary>

- restructured `rsrpp.parser`.
- updated how to extract section titles from PDF.
- updated tests.

</details>

<details>
<summary>1.0.16</summary>

- removed `init_logger` form `rsrpp`.

</details>

<details>
<summary>1.0.15</summary>

- fixed typo.
- introdeced `tracing` logger.

</details>

<details>
<summary>1.0.14</summary>

- Updated `rsrpp` version for `rsrpp-cli`.

</details>

<details>
<summary>1.0.13</summary>

- Updated dependencies.
- removed build.sh because it requires sudo when installing the crate.

</details>

<details>
<summary>1.0.12</summary>

- Fixed a bug: remove unused `println!`.

</details>

<details>
<summary>1.0.11</summary>

- Fixed a bug in xml loop to finish when the file reaches to end.

</details>

<details>
<summary>1.0.10</summary>

- Added verbose mode.
- Fixed a bug in the process extracting page number.

</details>

<details>
<summary>1.0.9</summary>

- Updated: implemented new errors to handle invalid URLs.

</details>

<details>
<summary>1.0.8</summary>

- Updated: The max retry time for saving PDF files has been increased.

</details>

<details>
<summary>1.0.7</summary>

- Fix bugs: After converting to PDF, the program now waits until processing is complete.

</details>

<details>
<summary>1.0.4</summary>

- Fixed bugs in `get_pdf_info`.
- Made minor improvements.

</details>

<details>
<summary>1.0.3</summary>

- Added cli -> [rsrpp-cli](https://crates.io/crates/rsrpp-cli).

</details>

<details>
<summary>1.0.2</summary>

- Updated the `Section` module. `content: String` was replaced by `content: Vec<TextBlock>`.

</details>
