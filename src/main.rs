//! `threshold` — session arrival briefing synthesizer.
//!
//! Gathers signals from multiple sources (recall, gossip, build manifest,
//! git, docket, review-due) and synthesizes them into a single prioritized
//! arrival briefing. Designed to replace the ~21 KB firehose of unsynthesized
//! `SessionStart` hook output.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use threshold::SourceSet;

fn main() -> std::process::ExitCode {
    sigpipe::reset();
    match run() {
        Ok(code) => code,
        Err(e) => {
            // Print to stderr is expected for CLI error reporting
            #[allow(clippy::print_stderr)]
            {
                eprintln!("threshold: error: {e:#}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<std::process::ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Brief(ref args) => cmd_brief(args),
    }
}

fn cmd_brief(args: &BriefArgs) -> Result<std::process::ExitCode> {
    let source_root = args.source_root.as_deref();
    let max_items = args.max_items;

    let sources = SourceSet::real(source_root);
    let signals = sources.collect_all();
    let briefing = threshold::synthesize(signals, max_items);

    // Print to stdout is the purpose of this CLI
    #[allow(clippy::print_stdout)]
    match args.format {
        Format::Text => {
            let text = briefing.render_text();
            print!("{text}");
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(&briefing)?;
            println!("{json}");
        }
    }
    Ok(std::process::ExitCode::SUCCESS)
}

#[derive(Parser)]
#[command(name = "threshold", about = "Session arrival briefing synthesizer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Gather signals and emit a synthesized arrival briefing
    Brief(BriefArgs),
}

#[derive(Parser)]
struct BriefArgs {
    /// Output format
    #[arg(long, default_value = "text")]
    format: Format,

    /// Maximum number of items per section (0 = unlimited)
    #[arg(long, default_value = "20")]
    max_items: usize,

    /// Override the root path for locating source data files (for testing)
    #[arg(long)]
    source_root: Option<PathBuf>,
}

#[derive(ValueEnum, Clone)]
enum Format {
    Text,
    Json,
}
