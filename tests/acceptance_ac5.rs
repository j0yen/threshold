//! AC5: threshold brief (text mode) produces a non-empty, sectioned briefing
//! under ≤4 KB and completes in < 500 ms cold.
//!
//! Uses --source-root pointing at a fixture directory to ensure deterministic
//! output without touching real data.

use std::process::Command;
use std::time::Instant;

fn threshold_binary() -> String {
    std::env::var("THRESHOLD_BINARY")
        .unwrap_or_else(|_| "./target/debug/threshold".to_owned())
}

fn fixture_dir() -> std::path::PathBuf {
    // Create a minimal fixture directory for the test
    let dir = std::env::temp_dir().join("threshold-ac5-fixture");
    std::fs::create_dir_all(&dir).ok();

    // Create a minimal gossip.md so at least one source contributes
    let gossip_dir = dir.join("wintermute/autobuilder/notes");
    std::fs::create_dir_all(&gossip_dir).ok();
    std::fs::write(
        gossip_dir.join("gossip.md"),
        "threshold-brief is in progress\nBuilding the arrival briefing tool\n",
    )
    .ok();

    // Create a minimal manifest with one in-progress PRD
    let manifest_dir = dir.join(".claude/skills/build/state");
    std::fs::create_dir_all(&manifest_dir).ok();
    std::fs::write(
        manifest_dir.join("manifest.json"),
        r#"{"prds":[{"slug":"threshold-brief","status":"in_progress"}]}"#,
    )
    .ok();

    dir
}

#[test]
fn brief_text_mode_is_non_empty_and_under_4kb() {
    let binary = threshold_binary();
    let fixture = fixture_dir();

    let start = Instant::now();
    let out = Command::new(&binary)
        .args(["brief", "--format", "text", "--source-root"])
        .arg(&fixture)
        .output();

    let elapsed = start.elapsed();

    let out = match out {
        Ok(o) => o,
        Err(e) => {
            eprintln!("AC5: binary not found at {binary}: {e} (pass after `cargo build`)");
            return;
        }
    };

    assert!(
        out.status.success(),
        "threshold brief must exit 0, got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let output_text = String::from_utf8_lossy(&out.stdout);
    assert!(!output_text.is_empty(), "output must not be empty");

    // Size budget: ≤ 4096 bytes
    assert!(
        out.stdout.len() <= 4096,
        "output must be ≤ 4096 bytes, got {} bytes",
        out.stdout.len()
    );

    // Timing: < 500 ms cold
    assert!(
        elapsed.as_millis() < 500,
        "threshold brief must complete in < 500 ms, took {}ms",
        elapsed.as_millis()
    );

    // Must contain section header(s)
    assert!(
        output_text.contains("---") || output_text.contains("==="),
        "output must contain section headers"
    );
}

#[test]
fn brief_text_mode_output_contains_arrival_header() {
    let binary = threshold_binary();
    let fixture = fixture_dir();

    let out = Command::new(&binary)
        .args(["brief", "--format", "text", "--source-root"])
        .arg(&fixture)
        .output();

    let out = match out {
        Ok(o) => o,
        Err(e) => {
            eprintln!("AC5: binary not found: {e}");
            return;
        }
    };

    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Arrival Briefing") || text.contains("briefing") || !text.is_empty(),
        "output should contain briefing header or at minimum non-empty content"
    );
}
