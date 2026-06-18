//! AC4 (shim): The shell shim exits 0 and emits nothing when the `threshold`
//! binary is absent or non-executable (test by running it with an empty PATH /
//! renamed binary).

use std::process::Command;

fn shim_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/jsy".to_owned());
    format!("{home}/.claude/scripts/threshold-session-start.sh")
}

/// AC4: Shim exits 0 and produces no output when PATH is empty (binary absent).
#[test]
fn shim_exits_0_with_empty_path() {
    let shim = shim_path();

    // Skip if shim doesn't exist yet (pre-install)
    if !std::path::Path::new(&shim).exists() {
        eprintln!("AC10: shim not found at {shim} — skip (will pass after install)");
        return;
    }

    let out = Command::new("bash")
        .args([&shim])
        .env_clear()
        // Restore HOME so bash can start; clear PATH so `threshold` is not found.
        .env("HOME", std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned()))
        .env("PATH", "")
        .output()
        .expect("bash must be available");

    assert_eq!(
        out.status.code(),
        Some(0),
        "shim must exit 0 when PATH is empty\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // Must produce no output
    assert!(
        out.stdout.is_empty(),
        "shim must emit nothing when binary is absent\nstdout: {}",
        String::from_utf8_lossy(&out.stdout),
    );
}
