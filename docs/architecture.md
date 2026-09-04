**English** | [日本語](architecture.ja.md)

# Architecture

A parse runs poppler four times — `pdfinfo` for the page geometry, `pdftocairo` for a
page image per page, `pdftohtml -xml` for the text with its coordinates, and
`pdftotext` for a second reading of the same text — and then decides, from the geometry
alone, which runs are body text, which are captions, and which are section headings.
The LLM is optional and never the only source of a decision.

## Core modules

- **`parser`**: Main PDF parsing logic
  - PDF to HTML conversion (using Poppler)
  - HTML structure parsing and page object generation
  - Section structure extraction

- **`models`**: Data structure definitions
  - `Word`: Word-level information (coordinates, font size, etc.)
  - `Line`: Line-level information and word collections
  - `Block`: Block-level information and line collections (with `BlockType`: Body, Caption, Header)
  - `Page`: Page-level information and block collections
  - `Section`: Section structure with `contents`, `math_contents`, and `captions` fields
  - `RichText`: Text with original and math-marked versions
  - `fix_suffix_hyphens`: Text normalization for compound words (e.g., "databased" → "data-based")

- **`cleaner`**: Text cleaning and block classification
  - Figure/table caption detection (e.g., "Figure 1:", "Table 2.")
  - Block type classification (Body, Caption, Header)

- **`llm`**: LLM-enhanced processing and math detection
  - Math expression detection using Unicode patterns, structural heuristics, and context analysis
  - LLM-based math extraction with trigram alignment to text blocks
  - False positive filtering (dates, statistics, section references)
  - Unicode-to-LaTeX conversion for unified `<math>...</math>` output
  - LLM-based section validation

- **`extracter`**: Figure and table extraction functionality
  - Figure detection using OpenCV
  - Table region identification and exclusion (with area cap to reject false positives)
  - Text area degenerate detection with full-page fallback

- **`converter`**: Format conversion functionality
  - Page to section conversion
  - Section detection with fallback for non-standard formats (anchor-word matching)
  - The watchdog around every poppler invocation, and the typed error it raises
  - JSON output generation

- **`config`**: Configuration management
  - Parser configuration management
  - The temporary directory a parse works in, and its lifetime
  - Math text mapping storage

- **`tempdir_sweep`**: Reclaims the working directories that killed runs left behind,
  once per process, and only where the owning process is provably gone

## How a section heading is decided

Font size alone cannot tell a section heading from a subplot title set in the same
size, and a false heading is not merely noise: it swallows the body that follows, so
the section it belongs to loses that text. Two passes are used.

The **font pass** collects every run set in a title font. The **placement pass** then
asks where each candidate sits: a heading answers to the page — an anchor word, a
column margin, a column centre, the indent the numbered headings share. Candidates that
answer to none of those are rejected.

The placement pass carries two safety valves, both of which were measured over a corpus
of 2,812 papers before being kept:

- It does not run at all below five candidates. There is not enough of the paper on
  show to read its layout from, and one wrong verdict would then be a large share of
  its sections.
- If its verdicts would reject half the candidates or more, the whole pass stands down.
  The rule and the document disagree about what the layout is, and the document is the
  authority.

Loosening either one removes no false headings and costs real ones.

## CLI tool

`rsrpp-cli` is an independent binary: command-line argument parsing, logging, JSON
output, and the [exit codes](cli.md#exit-codes) that let a batch run tell a hopeless
document from a broken environment.
