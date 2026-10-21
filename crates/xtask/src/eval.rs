use std::env;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use serde_json::Value;

use crate::bundle_layout::APP_EXECUTABLE;
use crate::finding::Finding;
use crate::workspace;

const CLASSES: [&str; 3] = ["compact", "standard", "extended"];
const CLASS_VARIABLE: &str = "DOMOVOI_EVAL_CLASS";
const PROXY_VARIABLE: &str = "DOMOVOI_MCP_PROXY";
const CLAUDE_CODE_TEST: &str = "claude_code::meets_the_eval_thresholds";

pub(crate) fn run(class: &str) -> io::Result<Vec<Finding>> {
    if !CLASSES.contains(&class) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown class {class:?}; use one of {CLASSES:?}"),
        ));
    }
    let status = Command::new(cargo())
        .args([
            "test",
            "-p",
            "domovoi-eval",
            "--release",
            "--test",
            "eval",
            "--",
            "--ignored",
            "--exact",
            "the_primary_model_of_a_class_meets_the_eval_thresholds",
            "--nocapture",
        ])
        .env(CLASS_VARIABLE, class)
        .current_dir(workspace::root())
        .status()?;
    finished(status, class)
}

pub(crate) fn run_claude_code() -> io::Result<Vec<Finding>> {
    let proxy = built_proxy()?;
    let status = Command::new(cargo())
        .args([
            "test",
            "-p",
            "domovoi-eval",
            "--features",
            "claude-code",
            "--test",
            "eval",
            "--",
            "--ignored",
            "--exact",
            CLAUDE_CODE_TEST,
            "--nocapture",
        ])
        .env(PROXY_VARIABLE, proxy)
        .current_dir(workspace::root())
        .status()?;
    finished(status, "Claude Code")
}

fn built_proxy() -> io::Result<PathBuf> {
    let mut child = Command::new(cargo())
        .args([
            "build",
            "-p",
            "domovoi-app",
            "--bin",
            APP_EXECUTABLE,
            "--features",
            "claude-code",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(workspace::root())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("cargo has no output"))?;
    let mut proxy = None;
    for line in BufReader::new(stdout).lines() {
        if let Ok(message) = serde_json::from_str::<Value>(&line?)
            && message["reason"] == "compiler-artifact"
            && message["target"]["name"] == APP_EXECUTABLE
            && let Some(executable) = message["executable"].as_str()
        {
            proxy = Some(PathBuf::from(executable));
        }
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "cargo build of {APP_EXECUTABLE} failed with {status}"
        )));
    }
    proxy.ok_or_else(|| io::Error::other(format!("the build did not report {APP_EXECUTABLE}")))
}

fn cargo() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn finished(status: ExitStatus, evaluated: &str) -> io::Result<Vec<Finding>> {
    if status.success() {
        Ok(Vec::new())
    } else {
        Err(io::Error::other(format!(
            "the {evaluated} evaluation failed with {status}"
        )))
    }
}
