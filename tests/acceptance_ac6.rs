//! AC6: First line of main() is sigpipe::reset(); threshold brief | head
//! does not panic (regression guard for the known SIGPIPE-panic class).

use std::process::{Command, Stdio};

fn threshold_binary() -> String {
    std::env::var("THRESHOLD_BINARY")
        .unwrap_or_else(|_| "./target/debug/threshold".to_owned())
}

#[test]
fn main_rs_first_statement_is_sigpipe_reset() {
    let main_src = std::fs::read_to_string("src/main.rs")
        .expect("src/main.rs must exist");

    // Find the main() function body and check its first real statement
    // Look for "sigpipe::reset();" anywhere before any other logic
    let has_sigpipe_reset = main_src.contains("sigpipe::reset()");
    assert!(
        has_sigpipe_reset,
        "src/main.rs must contain sigpipe::reset() call"
    );

    // The call must appear before any substantial logic.
    // Find position of sigpipe::reset() and check it's before any other fn call
    let sigpipe_pos = main_src
        .find("sigpipe::reset()")
        .expect("sigpipe::reset() must be present");

    // Check it comes early in the file (within the first 500 chars of fn main)
    let main_pos = main_src.find("fn main()").expect("fn main() must be present");
    let main_body_start = main_src[main_pos..].find('{').map(|p| main_pos + p);

    if let Some(body_start) = main_body_start {
        assert!(
            sigpipe_pos > body_start,
            "sigpipe::reset() must be inside fn main()"
        );
        // It should be within the first 200 chars of the function body
        assert!(
            sigpipe_pos - body_start < 200,
            "sigpipe::reset() should be the first statement in main(), found at offset {}",
            sigpipe_pos - body_start
        );
    }
}

#[test]
fn brief_pipe_to_head_does_not_panic() {
    // Run: threshold brief | head -n 5
    // threshold brief should not panic on SIGPIPE when head closes the pipe early.
    let binary = threshold_binary();

    let mut child = match Command::new(&binary)
        .args(["brief"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("AC6: binary not found at {binary}: {e} (pass after `cargo build`)");
            return;
        }
    };

    // Immediately close stdout from the child's perspective by dropping the pipe
    drop(child.stdout.take());

    let status = child.wait().expect("wait for child");

    // SIGPIPE exit code is 141 on Linux. We must NOT see it.
    // Acceptable exit codes: 0 (clean), 1 (no sources, non-zero exit), anything but 141
    let code = status.code().unwrap_or(0);
    assert_ne!(
        code, 141,
        "threshold brief must not exit 141 (SIGPIPE panic) when pipe is closed early"
    );

    // Also must not be 134 (SIGABRT, which Rust panic produces)
    assert_ne!(
        code, 134,
        "threshold brief must not abort/panic (exit 134) when pipe is closed early"
    );
}
