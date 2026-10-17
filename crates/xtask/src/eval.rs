use std::env;
use std::ffi::OsString;
use std::io;
use std::process::Command;

use crate::finding::Finding;
use crate::workspace;

const CLASSES: [&str; 3] = ["compact", "standard", "extended"];
const CLASS_VARIABLE: &str = "DOMOVOI_EVAL_CLASS";

pub(crate) fn run(class: &str) -> io::Result<Vec<Finding>> {
    if !CLASSES.contains(&class) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown class {class:?}; use one of {CLASSES:?}"),
        ));
    }
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let status = Command::new(cargo)
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
    if status.success() {
        Ok(Vec::new())
    } else {
        Err(io::Error::other(format!("the {class} evaluation failed with {status}")))
    }
}
