pub mod loggers;

use crate::loggers::init_logger;
use clap::Parser;
use rsrpp::parser::parse;
use rsrpp::parser::structs::{ParserConfig, Section};
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
}

#[tokio::main]
async fn main() {
    init_logger().expect("Failed to initialize logger");
    let args = Args::parse();

    let is_url = args.pdf.starts_with("http");
    if !is_url && !Path::new(args.pdf.as_str()).exists() {
        eprintln!("File not found: {}", args.pdf);
        std::process::exit(-1);
    }

    let outfile = args.out.unwrap_or("output.json".to_string());
    assert!(
        outfile.ends_with(".json"),
        "Output file must be a JSON file"
    );

    let mut config = ParserConfig::new();
    let pages = parse(args.pdf.as_str(), &mut config, args.verbose).await.unwrap();
    let sections = Section::from_pages(&pages);
    let json = serde_json::to_string_pretty(&sections).unwrap();

    std::fs::write(format!("{}", outfile), json).unwrap();
}
