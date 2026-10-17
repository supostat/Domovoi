#![forbid(unsafe_code)]

mod appcast;
mod build_outputs;
mod bundle;
mod bundle_layout;
mod bundle_profile;
mod changelog;
mod check_bundle;
mod check_comments;
mod check_sources;
mod check_tokens;
mod comment_scan;
mod copy_tree;
mod eval;
mod fetch_test_models;
mod finding;
mod git;
mod notarization;
mod property_list;
mod release;
mod signing;
mod sparkle_distribution;
mod test_model;
mod theme;
mod theme_codegen;
mod theme_stylesheet;
mod theme_tokens;
mod theme_value;
mod verified_download;
mod workspace;
mod workspace_version;

use std::process::ExitCode;

use finding::Finding;

const USAGE: &str = "usage: cargo xtask <check-comments | check-tokens | check-sources | theme [--check] | fetch-test-models | eval --class <compact|standard|extended> | bundle --profile <dev|adhoc|release> | check-bundle <app> | release>";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["check-comments"] => check_comments::run(),
        ["check-tokens"] => check_tokens::run(),
        ["check-sources"] => check_sources::run(),
        ["theme"] => theme::run(theme::Mode::Write),
        ["theme", "--check"] => theme::run(theme::Mode::Check),
        ["fetch-test-models"] => fetch_test_models::run(),
        ["eval", "--class", class] => eval::run(class),
        ["bundle", "--profile", profile] => bundle::run(profile),
        ["check-bundle", app] => check_bundle::run(app),
        ["release"] => release::run(),
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
