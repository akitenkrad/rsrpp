**English** | [日本語](usecases.ja.md)

# What it is for

RSRPP exists because reading a paper PDF as *text* is not enough. A pile of characters
in reading order loses which of them belong to the introduction and which to the
appendix, mixes figure axis labels into the argument, and pastes table cells into the
middle of a sentence. The parser's job is to hand back the structure that the PDF had
and the text extraction threw away.

## Turning a paper into notes

The output is one entry per section, each with its own body text and the captions of
the figures it contains. That maps directly onto a note per paper with a heading per
section, which is what the parser was built for: filling a reading archive without
retyping.

```bash
rsrpp --pdf "https://arxiv.org/pdf/1706.03762" --out attention.json
```

## Feeding a retrieval index

Chunking a paper by character count cuts across arguments. Chunking it by section does
not, and it gives every chunk a title worth storing alongside the vector. `contents` is
already free of captions and table cells, so a chunk is prose rather than prose with
axis labels stirred in.

## Running over a corpus

The parser is built to be pointed at thousands of PDFs rather than one. That is why the
failure behaviour is as specific as it is: a document that will never finish is stopped
and reported with [its own exit code](cli.md#exit-codes) so a batch can skip it, the
temporary files of one paper cannot slow down the next, and every parse states how much
of the source text it kept so a silently mangled paper is visible without diffing the
output.

## Reading the mathematics

Math in a PDF is a mixture of Unicode symbols, ASCII conventions and images. RSRPP
marks what it finds with `<math>...</math>` and unifies it to LaTeX, so a downstream
tool sees one notation rather than three. See [Output format](output.md#math-markup).

## What it is not

- **Not an OCR engine.** A scanned page with no text layer yields nothing; run OCR
  first.
- **Not a layout-faithful converter.** The output is structured text, not a
  reproduction of the page.
- **Not dependent on an LLM.** The LLM path improves math extraction and section
  validation, but every structural decision has a geometric answer underneath it, and
  `--no-llm` is a supported way to run.
