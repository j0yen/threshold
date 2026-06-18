//! `threshold` — session arrival briefing synthesizer.
//!
//! Gathers signals from multiple sources (recall, gossip, build manifest,
//! git, docket, review-due, ledger) and synthesizes them into a single
//! prioritized arrival briefing. Also provides an append-only question
//! ledger for predecessor→successor session hand-off, and `threshold verify`
//! which parses the latest reflective letter and cross-checks each claim
//! against live ground truth.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use threshold::SourceSet;
use threshold::verify::{VerifyOptions, extract_claims, render_text, verify_claims};

/// Internal deadline for the `--hook` mode (5 seconds).
const HOOK_DEADLINE: Duration = Duration::from_secs(5);

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
        Commands::Verify(ref args) => cmd_verify(args),
    }
}

// ─── brief ────────────────────────────────────────────────────────────────────

fn cmd_brief(args: &BriefArgs) -> Result<std::process::ExitCode> {
    if args.hook {
        return cmd_brief_hook(args);
    }

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

/// SessionStart hook mode: synthesize briefing, record arrival, always exit 0.
///
/// This function is intentionally infallible from the caller's perspective —
/// all errors are swallowed so the hook never blocks session startup.
fn cmd_brief_hook(args: &BriefArgs) -> Result<std::process::ExitCode> {
    // Resolve the ledger path (use override if provided, else default).
    let ledger_path = threshold::ledger::ledger_path(args.ledger.as_deref());

    // Resolve session ID: prefer $CLAUDE_SESSION_ID, fall back to RealIdSource.
    let session_id = resolve_hook_session_id();

    // Record arrival — best-effort; failure must never abort the hook.
    let _ = threshold::record_arrival(&ledger_path, &session_id);

    // Collect signals with an internal deadline so we never hang.
    let source_root = args.source_root.as_deref();
    let sources = SourceSet::real(source_root);
    let signals = sources.collect_with_deadline(HOOK_DEADLINE);
    let briefing = threshold::synthesize(signals, args.max_items);

    // Print to stdout is the purpose of this CLI
    #[allow(clippy::print_stdout)]
    {
        let text = briefing.render_text();
        print!("{text}");
    }

    // ALWAYS exit 0 — this is a hook; it must never fail session startup.
    Ok(std::process::ExitCode::SUCCESS)
}

/// Resolve session ID for hook mode.
///
/// Prefers the `$CLAUDE_SESSION_ID` environment variable (set by the harness),
/// then falls back to the agentns / hostname:pid path.
fn resolve_hook_session_id() -> String {
    if let Ok(sid) = std::env::var("CLAUDE_SESSION_ID") {
        if !sid.trim().is_empty() {
            return sid.trim().to_owned();
        }
    }
    resolve_session_id()
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

// ─── verify ───────────────────────────────────────────────────────────────────

fn cmd_verify(args: &VerifyArgs) -> Result<std::process::ExitCode> {
    // Locate the reflective note to parse.
    let letter_text = load_letter(args)?;

    let opts = VerifyOptions {
        source_root: args.source_root.clone(),
    };

    let claims = extract_claims(&letter_text);
    let verdicts = verify_claims(&claims, &opts);

    // Print to stdout is the purpose of this CLI
    #[allow(clippy::print_stdout)]
    match args.format {
        Format::Text => {
            let text = render_text(&verdicts);
            print!("{text}");
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(&verdicts)?;
            println!("{json}");
        }
    }

    Ok(std::process::ExitCode::SUCCESS)
}

/// Load the letter text from the recall reflective store or the fixture seam.
fn load_letter(args: &VerifyArgs) -> Result<String> {
    if let Some(ref note_id) = args.note {
        // Specific note ID: `recall get <id>`
        return recall_get(note_id);
    }

    // Default: try `recall list --kind reflective --limit 1 --format json` and
    // get the body of the latest entry.
    let latest = recall_latest_reflective(args.source_root.as_deref())?;
    Ok(latest)
}

/// Fetch a recall note body by ID via `recall show <id>`.
fn recall_get(id: &str) -> Result<String> {
    let out = std::process::Command::new("recall")
        .args(["show", id])
        .output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(anyhow::anyhow!("recall show {id} failed: {stderr}"))
    }
}

/// Get the body of the latest `reflective/self` recall note.
///
/// Uses `recall list --kind reflective --limit 1` to get the ID, then
/// `recall show <id>` for the full body.
/// Falls back to reading the raw reflective log tail when `recall` fails
/// (e.g. no `recall` binary on `PATH`).
fn recall_latest_reflective(source_root: Option<&std::path::Path>) -> Result<String> {
    // Step 1: get the ID of the latest reflective note.
    let list_out = std::process::Command::new("recall")
        .args(["list", "--kind", "reflective", "--limit", "1"])
        .output();

    if let Ok(o) = list_out {
        if o.status.success() {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // Output is like: "01KVCTMDKY25939E4D3AGDNN7E  [reflective/self]  ..."
            // Extract the first word (the ID).
            if let Some(id) = stdout.split_whitespace().next() {
                // Step 2: get the full body via `recall show <id>`.
                let show_out = std::process::Command::new("recall")
                    .args(["show", id])
                    .output();
                if let Ok(so) = show_out {
                    if so.status.success() {
                        return Ok(String::from_utf8_lossy(&so.stdout).into_owned());
                    }
                }
            }
        }
    }

    // Fallback: read the last 200 lines of the reflective log file.
    let log_path = source_root.map_or_else(
        || {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_owned());
            std::path::PathBuf::from(home).join(".local/share/recall/reflective.log")
        },
        |root| root.join("recall/reflective.log"),
    );

    let content = std::fs::read_to_string(&log_path).unwrap_or_default();
    let last_lines: String = content
        .lines()
        .rev()
        .take(200)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");

    Ok(last_lines)
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
    /// Parse the latest reflective letter and cross-check each claim against live ground truth
    Verify(VerifyArgs),
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

    /// SessionStart hook mode: always exits 0, time-bounded, records arrival in ledger
    #[arg(long)]
    hook: bool,

    /// Override ledger file path (for testing hook mode)
    #[arg(long, hide = true)]
    ledger: Option<PathBuf>,
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

#[derive(Parser)]
struct VerifyArgs {
    /// Output format
    #[arg(long, default_value = "text")]
    format: Format,

    /// Target a specific recall note by ID (default: latest reflective note)
    #[arg(long)]
    note: Option<String>,

    /// Override the root path for locating source data files (for testing)
    #[arg(long)]
    source_root: Option<PathBuf>,
}

#[derive(ValueEnum, Clone)]
enum Format {
    Text,
    Json,
}
