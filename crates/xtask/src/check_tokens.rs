use std::fs;
use std::io;
use std::path::Path;

use crate::finding::Finding;
use crate::workspace;

const HEX_COLOR_DIGITS: usize = 6;
const RGBA_CALL: &str = "rgba(";
const CRATES_ALLOWED_TO_HOLD_COLORS: [&str; 2] = ["theme", "xtask"];

pub(crate) fn run() -> io::Result<Vec<Finding>> {
    let crates_dir = workspace::crates_dir();
    let excluded: Vec<_> = CRATES_ALLOWED_TO_HOLD_COLORS
        .iter()
        .map(|name| crates_dir.join(name))
        .collect();
    let mut findings = Vec::new();
    for path in workspace::source_files(&crates_dir, &excluded)? {
        findings.extend(findings_in_file(&path)?);
    }
    Ok(findings)
}

fn findings_in_file(path: &Path) -> io::Result<Vec<Finding>> {
    let source = fs::read_to_string(path)?;
    let findings = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            color_literal(line).map(|literal| Finding {
                path: path.to_path_buf(),
                line: index + 1,
                reason: format!("color literal `{literal}` outside the theme crate"),
            })
        })
        .collect();
    Ok(findings)
}

fn color_literal(line: &str) -> Option<String> {
    if line.contains(RGBA_CALL) {
        return Some(RGBA_CALL.to_owned());
    }
    let bytes = line.as_bytes();
    bytes.iter().enumerate().find_map(|(index, byte)| {
        let digits = bytes.get(index + 1..index + 1 + HEX_COLOR_DIGITS)?;
        (*byte == b'#' && digits.iter().all(u8::is_ascii_hexdigit))
            .then(|| String::from_utf8_lossy(&bytes[index..=index + HEX_COLOR_DIGITS]).into_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::color_literal;

    #[test]
    fn flags_hex_colors_and_rgba_calls() {
        assert_eq!(color_literal("let wax = \"#E3AE52\";"), Some("#E3AE52".to_owned()));
        assert_eq!(color_literal("background: rgba(0,0,0,.5)"), Some("rgba(".to_owned()));
    }

    #[test]
    fn passes_attributes_and_raw_strings() {
        assert_eq!(color_literal("#[derive(Debug)]"), None);
        assert_eq!(color_literal("let raw = r#\"text\"#;"), None);
        assert_eq!(color_literal("let short = \"#abc\";"), None);
    }
}
