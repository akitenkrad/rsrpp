# rsrpp — Rust Research Paper Parser

![Crates.io Version](https://img.shields.io/crates/v/rsrpp?style=flat-square)
![License: MIT](https://img.shields.io/crates/l/rsrpp?style=flat-square)

`rsrpp` turns a research paper PDF into structured sections. It drives poppler for the
text and its geometry, works out the column layout and which lines are section headings,
and returns sections carrying body text, figure and table captions, math, and optionally
references.

## Requirements

```bash
sudo apt install poppler-utils libopencv-dev clang libclang-dev   # Ubuntu/Debian
brew install poppler opencv pkg-config                            # macOS
```

OpenCV 4.x is needed only by the default `table-detection` feature. Building with
`--no-default-features` removes the OpenCV dependency entirely, at the cost of table
regions no longer being excluded from the body text.

## Usage

```rust
use rsrpp::{config::ParserConfig, models::Section, parser::parse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ParserConfig::new();
    let pages = parse("https://arxiv.org/pdf/1706.03762", &mut config, false).await?;

    let sections = Section::from_pages(&pages);
    println!("{}", serde_json::to_string_pretty(&sections)?);
    Ok(())
}
```

## What every parse tells you

Text inside a detected table region, and blocks too narrow and short to be body text,
are excluded on purpose. Nothing is excluded silently: every parse reports its coverage,
and `ParserConfig::dropped_texts` holds each removed fragment with the page it came from
and the reason it went.

```
Text coverage 93.5% (32142 of 34364 source chars kept). Discarded — narrow block: 210
fragments / 1142 chars, outside text area: 2 fragments / 62 chars, table region: 142
fragments / 1018 chars.
```

## Temporary files

A parse works in a directory of its own and removes it when the last handle to the
config goes away. **A path copied out of `config.pdf_figures` does not outlive the
config** — read the file while the config is alive. A process killed with `SIGKILL` runs
no destructor, so the next parse reclaims what earlier runs abandoned.

## Limits on poppler

Some PDFs make poppler write without bound — a figure painted with a one-point tiling
pattern has `pdftohtml` emit a PNG per tile, which is millions of files from one parse.
Every poppler call is watched and killed if it passes a time limit or an output limit,
and a breach comes back as `converter::PopplerLimitError` so callers can recognise it by
type rather than by matching on a message.

## Documentation

- [Library guide](https://github.com/akitenkrad/rsrpp/blob/main/docs/library.md)
- [Output format](https://github.com/akitenkrad/rsrpp/blob/main/docs/output.md)
- [Architecture](https://github.com/akitenkrad/rsrpp/blob/main/docs/architecture.md)
- [Release history](https://github.com/akitenkrad/rsrpp/blob/main/docs/releases.md)
- [日本語のドキュメント](https://github.com/akitenkrad/rsrpp/blob/main/README.ja.md)

## License

MIT.
