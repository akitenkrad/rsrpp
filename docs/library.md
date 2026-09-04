**English** | [日本語](library.ja.md)

# Library

```rust
use rsrpp::{config::ParserConfig, models::Section, parser::parse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ParserConfig::new(); // LLM enabled by default
    let verbose = true;

    // Specify URL or local file path
    let url = "https://arxiv.org/pdf/1706.03762";

    // Parse PDF and get page structure
    let pages = parse(url, &mut config, verbose).await?;

    // Convert to section structure (basic)
    let sections = Section::from_pages(&pages);

    // Or with math markup (math expressions wrapped in <math>...</math> tags)
    let sections_with_math = Section::from_pages_with_math(&pages, &config.math_texts);

    // Output in JSON format
    let json = serde_json::to_string_pretty(&sections_with_math)?;
    println!("{}", json);

    Ok(())
}
```

## Temporary files

A parse works in a directory of its own under the system temp directory, and
**everything it produced is removed when the last handle to the config goes away** —
on the normal path, on an early return, and while a panic unwinds. `clean_files()`
still exists and removes the same directory; calling it is now optional, and calling
it twice is not an error.

Two consequences worth knowing:

- **Paths taken out of the config do not outlive it.** If you copy a path out of
  `config.pdf_figures` and read the file after the config has been dropped, it is
  gone. Copy the file out while the config is alive.
- **A killed process cannot clean up after itself.** `SIGKILL` runs no destructor, so
  a run that is killed leaves its directory behind, and poppler may outlive its parent
  and keep writing into it. The next parse in the same temp directory reclaims what
  earlier runs abandoned — only directories whose owning process is provably gone, so
  parses running side by side never take each other's files. `sweep_abandoned_temp_dirs()`
  is public if you would rather trigger that yourself.

## Limits on poppler

Some PDFs make poppler write without bound. One 17-page paper in the wild, whose figure
is painted with tiling patterns one and four points wide, makes `pdftohtml` emit a PNG
per tile: over a million files and 7.4 GB from a single parse, with no end in sight.
Left alone, that one document stops a whole corpus run.

Every poppler invocation is therefore watched, and killed if it passes either of two
limits. A time limit alone would not do: a 1,000-page scanned proceedings is slow for
an honest reason, and "slow" and "unbounded" have to be told apart. So the output is
bounded too, per page of the document:

| Field | Default | Constant |
|---|---|---|
| `poppler_timeout` | 900s | `DEFAULT_POPPLER_TIMEOUT` |
| `poppler_max_files_per_page` | 200 | `DEFAULT_POPPLER_MAX_FILES_PER_PAGE` |
| `poppler_max_bytes_per_page` | 10 MiB | `DEFAULT_POPPLER_MAX_BYTES_PER_PAGE` |

Short documents get a floor rather than a proportional budget, so that a two-page paper
is not held to 400 files: `MIN_POPPLER_FILE_BUDGET` (2,000 files) and
`MIN_POPPLER_BYTE_BUDGET` (200 MiB).

The defaults leave a wide margin over what healthy documents need — a 357-page scan
measures about 4 files and 470 KB per page — so raising them is rarely the answer to a
paper that will not parse. Lower them if you are running untrusted PDFs and want to
fail faster:

```rust
use std::time::Duration;

let mut config = ParserConfig::new();
config.poppler_timeout = Duration::from_secs(120);
config.poppler_max_files_per_page = 50;
```

## Error handling

A limit breach is reported as a typed error, so callers can recognise it without
matching on the message text:

```rust
use rsrpp::converter::{PopplerLimit, PopplerLimitError};

match parse(url, &mut config, false).await {
    Ok(pages) => { /* ... */ }
    Err(e) => {
        match e.downcast_ref::<PopplerLimitError>().map(|e| e.limit()) {
            Some(PopplerLimit::Time) => eprintln!("this document is too slow to finish"),
            Some(PopplerLimit::Files) | Some(PopplerLimit::Bytes) => {
                eprintln!("this document makes poppler write without bound")
            }
            None => eprintln!("parsing error: {}", e),
        }
    }
}
```

`PopplerLimit` is `#[non_exhaustive]`: a fourth limit could be added without breaking
the `match` above.

## What was discarded

Nothing is discarded silently. Every removed fragment is in
`ParserConfig::dropped_texts` with the page it came from and the reason it went:

```rust
let pages = parse(url, &mut config, false).await?;

println!("coverage: {:?}", config.coverage());
for dropped in &config.dropped_texts {
    println!("p{} [{}] {}", dropped.page, dropped.reason, dropped.text);
}
```

See [Text coverage](cli.md#text-coverage) for what the coverage number counts.

## Custom configuration

```rust
use rsrpp::config::ParserConfig;

let mut config = ParserConfig::new();
config.use_llm = false;             // skip the LLM path entirely
config.extract_references = true;   // structured references (needs OPENAI_API_KEY)

let pages = parse("path/to/paper.pdf", &mut config, true).await?;
```

`ParserConfig` is `#[non_exhaustive]`: build it with `new()` and adjust the fields you
care about, rather than with a struct literal.
