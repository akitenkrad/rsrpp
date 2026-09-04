pub mod loggers;

use crate::loggers::init_logger;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use rsrpp::{
    config::ParserConfig,
    converter::PopplerLimitError,
    models::Section,
    parser::{pages2paper_output, parse, unassigned_section},
};
use std::path::Path;
use std::process::ExitCode;

/// The parse succeeded and the JSON was written.
const EXIT_OK: u8 = 0;

/// Something about this invocation was wrong, or the environment is: the PDF is missing
/// or broken, `--out` cannot be written, poppler is not installed. A corpus run should
/// look at these rather than skip past them.
const EXIT_FAILURE: u8 = 1;

/// poppler hit one of its ceilings on this document (see `PopplerLimitError`). Nothing
/// is wrong with the installation -- this one document is too large or makes poppler
/// write without bound -- so a corpus run may record it and move to the next paper.
///
/// Not 2: clap already exits 2 for a usage error, and a caller keying on "give up"
/// must not confuse "this paper is hopeless" with "I passed the wrong flags".
const EXIT_GIVE_UP: u8 = 3;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    pdf: String,

    #[arg(short, long)]
    out: Option<String>,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Disable LLM-enhanced processing"
    )]
    no_llm: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Include captions in the main contents field instead of separate captions field"
    )]
    include_captions: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Disable math markup (skip math detection and tagging)"
    )]
    no_math_markup: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Extract structured references (requires OPENAI_API_KEY)"
    )]
    extract_references: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "Append an 'Unassigned' section holding every fragment the parsing filters discarded (table cells, figure labels), making the output lossless"
    )]
    keep_dropped: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(e) = init_logger() {
        eprintln!("rsrpp: failed to initialize logger: {}", one_line(&e));
        return ExitCode::from(EXIT_FAILURE);
    }
    let args = Args::parse();

    // `run` owns the `ParserConfig`, so its temporary directory is removed by `Drop`
    // whichever way this returns. Exiting from inside it -- `process::exit`, a panic
    // with the parse still on the stack -- is what would leave the directory behind.
    match run(args).await {
        Ok(()) => ExitCode::from(EXIT_OK),
        Err(e) => {
            // One line, no backtrace: a corpus run of 1,863 papers has to stay readable,
            // and the exit code, not the text, is what the caller branches on.
            eprintln!("rsrpp: {}", one_line(&e));
            ExitCode::from(exit_code_for(&e))
        }
    }
}

/// Which exit code a failure deserves.
///
/// The one distinction that matters to a caller processing a corpus: a document poppler
/// gave up on can be skipped, anything else wants looking at. The typed error is what
/// separates them -- matching on the message text would break the first time the wording
/// changed.
fn exit_code_for(err: &anyhow::Error) -> u8 {
    if err.downcast_ref::<PopplerLimitError>().is_some() {
        EXIT_GIVE_UP
    } else {
        EXIT_FAILURE
    }
}

/// Flattens an error and its causes into a single line.
///
/// Newlines are folded out because poppler's stderr can end up inside the message, and a
/// diagnostic that spans lines is one a batch log cannot be grepped by.
fn one_line(err: &anyhow::Error) -> String {
    err.chain()
        .map(|cause| cause.to_string().replace('\n', " "))
        .collect::<Vec<_>>()
        .join(": ")
}

async fn run(args: Args) -> Result<()> {
    let is_url = args.pdf.starts_with("http");
    if !is_url && !Path::new(args.pdf.as_str()).exists() {
        return Err(anyhow!("File not found: {}", args.pdf));
    }

    let outfile = args.out.unwrap_or_else(|| "output.json".to_string());
    if !outfile.ends_with(".json") {
        return Err(anyhow!("Output file must be a JSON file: {}", outfile));
    }

    let mut config = ParserConfig::new();
    if args.no_llm {
        config.use_llm = false;
    }
    if args.extract_references {
        config.extract_references = true;
    }
    let pages = parse(args.pdf.as_str(), &mut config, args.verbose).await?;

    // Output format depends on whether references are extracted
    let json = if args.extract_references {
        // Use PaperOutput format with sections and references
        let mut output = pages2paper_output(&pages, &config);

        // Optionally merge captions into contents
        if args.include_captions {
            for section in &mut output.sections {
                if !section.captions.is_empty() {
                    section.contents.extend(section.captions.drain(..));
                }
            }
        }

        // Optionally strip math markup
        if args.no_math_markup {
            for section in &mut output.sections {
                section.math_contents = None;
            }
        }

        if args.keep_dropped {
            output.sections.extend(unassigned_section(&config));
        }

        serde_json::to_string_pretty(&output).context("Failed to serialize the parsed paper")?
    } else {
        // Generate sections with or without math markup
        let mut sections = if args.no_math_markup {
            Section::from_pages(&pages)
        } else {
            Section::from_pages_with_math(&pages, &config.math_texts)
        };

        // Optionally merge captions into contents
        if args.include_captions {
            for section in &mut sections {
                if !section.captions.is_empty() {
                    section.contents.extend(section.captions.drain(..));
                }
            }
        }

        if args.keep_dropped {
            sections.extend(unassigned_section(&config));
        }

        serde_json::to_string_pretty(&sections).context("Failed to serialize the parsed sections")?
    };

    std::fs::write(&outfile, json).with_context(|| format!("Failed to write {}", outfile))?;

    Ok(())
}
