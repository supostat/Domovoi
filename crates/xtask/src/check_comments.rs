use std::fs;
use std::io;
use std::path::Path;

use crate::comment_scan;
use crate::finding::Finding;
use crate::workspace;

const FORBIDDEN_REFERENCES: [&str; 5] = ["design-book", "книг", "phase ", "фаза ", "spec-"];

pub(crate) fn run() -> io::Result<Vec<Finding>> {
    let mut findings = Vec::new();
    for path in workspace::source_files(&workspace::crates_dir(), &[])? {
        findings.extend(findings_in_file(&path)?);
    }
    Ok(findings)
}

fn findings_in_file(path: &Path) -> io::Result<Vec<Finding>> {
    let source = fs::read_to_string(path)?;
    let findings = comment_scan::comments(&source)
        .into_iter()
        .filter_map(|comment| {
            forbidden_reference(&comment.text).map(|reference| Finding {
                path: path.to_path_buf(),
                line: comment.line,
                reason: format!("comment references `{reference}`: {}", comment.text.trim()),
            })
        })
        .collect();
    Ok(findings)
}

fn forbidden_reference(comment_text: &str) -> Option<&'static str> {
    let lowered = comment_text.to_lowercase();
    FORBIDDEN_REFERENCES
        .into_iter()
        .find(|reference| lowered.contains(reference))
}

#[cfg(test)]
mod tests {
    use super::forbidden_reference;

    #[test]
    fn flags_references_to_the_book_phases_and_spec() {
        assert_eq!(forbidden_reference("// see design-book section V"), Some("design-book"));
        assert_eq!(forbidden_reference("// как в Книге"), Some("книг"));
        assert_eq!(forbidden_reference("// Phase 3 adds this"), Some("phase "));
        assert_eq!(forbidden_reference("// Фаза сканера"), Some("фаза "));
        assert_eq!(forbidden_reference("// per SPEC-DOMOVOI"), Some("spec-"));
    }

    #[test]
    fn passes_ordinary_comments() {
        assert_eq!(forbidden_reference("// SAFETY: the buffer outlives the call"), None);
    }
}
