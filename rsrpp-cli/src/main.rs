pub mod loggers;

use crate::loggers::init_logger;
use clap::Parser;
use rsrpp::{
    config::ParserConfig,
    models::Section,
    parser::{pages2paper_output, parse},
};
use std::path::Path;

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
}

#[tokio::main]
async fn main() {
    init_logger().expect("Failed to initialize logger");
    let args = Args::parse();

    let is_url = args.pdf.starts_with("http");
    if !is_url && !Path::new(args.pdf.as_str()).exists() {
        tracing::error!("File not found: {}", args.pdf);
        std::process::exit(-1);
    }

    let outfile = args.out.unwrap_or("output.json".to_string());
    assert!(
        outfile.ends_with(".json"),
        "Output file must be a JSON file"
    );

    let mut config = ParserConfig::new();
    if args.no_llm {
        config.use_llm = false;
    }
    if args.extract_references {
        config.extract_references = true;
    }
    let pages = parse(args.pdf.as_str(), &mut config, args.verbose).await.unwrap();

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

        serde_json::to_string_pretty(&output).unwrap()
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

        serde_json::to_string_pretty(&sections).unwrap()
    };

    std::fs::write(format!("{}", outfile), json).unwrap();
}
