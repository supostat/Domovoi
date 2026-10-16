#![forbid(unsafe_code)]

mod check_comments;
mod check_sources;
mod check_tokens;
mod comment_scan;
mod finding;
mod workspace;

use std::process::ExitCode;

use finding::Finding;

const USAGE: &str = "usage: cargo xtask <check-comments | check-tokens | check-sources>";

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let outcome = match task.as_deref() {
        Some("check-comments") => check_comments::run(),
        Some("check-tokens") => check_tokens::run(),
        Some("check-sources") => check_sources::run(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match outcome {
        Ok(findings) => report(&findings),
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

fn report(findings: &[Finding]) -> ExitCode {
    if findings.is_empty() {
        return ExitCode::SUCCESS;
    }
    for finding in findings {
        eprintln!("{finding}");
    }
    eprintln!("xtask: {} finding(s)", findings.len());
    ExitCode::FAILURE
}
