//! `threshold` — session arrival briefing synthesizer.
//!
//! Gathers signals from multiple sources (recall, gossip, build manifest,
//! git, docket, review-due, ledger) and synthesizes them into a single
//! prioritized arrival briefing. Also provides an append-only question
//! ledger for predecessor→successor session hand-off.

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
        Commands::Ask(ref args) => cmd_ask(args),
        Commands::Answer(ref args) => cmd_answer(args),
        Commands::Open(ref args) => cmd_open(args),
    }
}

// ─── brief ────────────────────────────────────────────────────────────────────

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

// ─── ask ──────────────────────────────────────────────────────────────────────

fn cmd_ask(args: &AskArgs) -> Result<std::process::ExitCode> {
    let ledger_path = threshold::ledger::ledger_path(args.ledger.as_deref());
    let session_id = resolve_session_id();
    let tags: Vec<String> = args
        .tags
        .as_deref()
        .map(|t| t.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let id = threshold::ledger::ask(&ledger_path, &session_id, &args.question, tags)?;

    // Print to stdout is the purpose of this CLI
    #[allow(clippy::print_stdout)]
    {
        println!("Asked: {id}");
    }
    Ok(std::process::ExitCode::SUCCESS)
}

// ─── answer ───────────────────────────────────────────────────────────────────

fn cmd_answer(args: &AnswerArgs) -> Result<std::process::ExitCode> {
    let ledger_path = threshold::ledger::ledger_path(args.ledger.as_deref());
    let session_id = resolve_session_id();

    threshold::ledger::answer(&ledger_path, &session_id, &args.id, &args.answer)?;

    // Print to stdout is the purpose of this CLI
    #[allow(clippy::print_stdout)]
    {
        println!("Answered: {}", args.id);
    }
    Ok(std::process::ExitCode::SUCCESS)
}

// ─── open ─────────────────────────────────────────────────────────────────────

fn cmd_open(args: &OpenArgs) -> Result<std::process::ExitCode> {
    let ledger_path = threshold::ledger::ledger_path(args.ledger.as_deref());
    let questions = threshold::ledger::open_questions(&ledger_path)?;

    // Print to stdout is the purpose of this CLI
    #[allow(clippy::print_stdout)]
    match args.format {
        Format::Text => {
            if questions.is_empty() {
                println!("(no open questions)");
            } else {
                println!("Open questions ({}):", questions.len());
                for q in &questions {
                    println!("  [{}] {}", q.id, q.question);
                    if !q.tags.is_empty() {
                        println!("      tags: {}", q.tags.join(", "));
                    }
                    println!("      asked by: {}", q.asked_by_session);
                }
            }
        }
        Format::Json => {
            let output = threshold::OpenQuestionsOutput {
                schema: "threshold.ledger.open.v1".to_owned(),
                open_questions: questions,
            };
            let json = serde_json::to_string_pretty(&output)?;
            println!("{json}");
        }
    }
    Ok(std::process::ExitCode::SUCCESS)
}

// ─── Session ID resolution ────────────────────────────────────────────────────

fn resolve_session_id() -> String {
    let src = threshold::session_id::RealIdSource;
    threshold::session_id::resolve(&src).to_string()
}

// ─── CLI types ────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "threshold",
    about = "Session arrival briefing synthesizer and question ledger"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Gather signals and emit a synthesized arrival briefing
    Brief(BriefArgs),
    /// Leave a question for the next session
    Ask(AskArgs),
    /// Answer a predecessor's open question
    Answer(AnswerArgs),
    /// List unanswered questions left by predecessors
    Open(OpenArgs),
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

#[derive(Parser)]
struct AskArgs {
    /// The question to leave for the next session
    question: String,

    /// Comma-separated tags (optional)
    #[arg(long)]
    tags: Option<String>,

    /// Override ledger file path (for testing)
    #[arg(long, hide = true)]
    ledger: Option<PathBuf>,
}

#[derive(Parser)]
struct AnswerArgs {
    /// The question ID to answer (from `threshold open`)
    id: String,

    /// The answer text
    answer: String,

    /// Override ledger file path (for testing)
    #[arg(long, hide = true)]
    ledger: Option<PathBuf>,
}

#[derive(Parser)]
struct OpenArgs {
    /// Output format
    #[arg(long, default_value = "text")]
    format: Format,

    /// Override ledger file path (for testing)
    #[arg(long, hide = true)]
    ledger: Option<PathBuf>,
}

#[derive(ValueEnum, Clone)]
enum Format {
    Text,
    Json,
}
