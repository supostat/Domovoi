use std::fs;
use std::io;
use std::path::Path;

use crate::finding::Finding;
use crate::workspace;

struct StaleStatement {
    relative_path: &'static str,
    text: &'static str,
    why: &'static str,
}

const STALE_STATEMENTS: [StaleStatement; 4] = [
    StaleStatement {
        relative_path: "design/domovoi-design-book.html",
        text: "Решение открыто",
        why: "the open decisions were closed by the plan",
    },
    StaleStatement {
        relative_path: "design/domovoi-design-book.html",
        text: "SDF для чёткости",
        why: "diagram text uses a raster glyph atlas, not SDF",
    },
    StaleStatement {
        relative_path: "docs/DOMOVOI.md",
        text: "(dev, inode)",
        why: "identity keys are REALDEVID with FILEID, CLONEID and LINKID",
    },
    StaleStatement {
        relative_path: "docs/DOMOVOI.md",
        text: "metallib встроена",
        why: "precompiled metallib files ship inside the service bundle",
    },
];

const REMOVAL_CALLS: [&str; 5] = [
    "remove_file(",
    "remove_dir(",
    "remove_dir_all(",
    "unlink(",
    "removeItem",
];
const TEST_MODULE: &str = "#[cfg(test)]";
const TEST_DIRECTORY: &str = "tests";
const TEST_SUPPORT_CRATE: &str = "testkit";
const THIS_CHECK: &str = "xtask/src/check_sources.rs";
const PURGE_MODULE: &str = "rules/src/trash_purge.rs";
const MCP_SOCKET_MODULE: &str = "agent/src/private_socket.rs";

pub(crate) fn run() -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    let mut findings = Vec::new();
    for statement in &STALE_STATEMENTS {
        let path = root.join(statement.relative_path);
        let content = fs::read_to_string(&path)?;
        findings.extend(occurrences(&path, &content, statement));
    }
    findings.extend(removal_calls()?);
    Ok(findings)
}

fn removal_calls() -> io::Result<Vec<Finding>> {
    let crates = workspace::crates_dir();
    let excluded = [crates.join(TEST_SUPPORT_CRATE)];
    let mut findings = Vec::new();
    for path in workspace::source_files(&crates, &excluded)? {
        let content = fs::read_to_string(&path)?;
        findings.extend(removal_findings(&path, &content));
    }
    Ok(findings)
}

fn removal_findings(path: &Path, content: &str) -> Vec<Finding> {
    let in_tests = path
        .components()
        .any(|component| component.as_os_str() == TEST_DIRECTORY);
    let removes_own_files = [THIS_CHECK, PURGE_MODULE, MCP_SOCKET_MODULE]
        .iter()
        .any(|module| path.ends_with(module));
    if in_tests || removes_own_files {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if line.trim() == TEST_MODULE {
            break;
        }
        if let Some(call) = REMOVAL_CALLS.iter().find(|call| line.contains(*call)) {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: index + 1,
                reason: format!(
                    "{call} outside tests, the purge module and the MCP socket: files leave only through the Trash"
                ),
            });
        }
    }
    findings
}

fn occurrences(path: &Path, content: &str, statement: &StaleStatement) -> Vec<Finding> {
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(statement.text))
        .map(|(index, _)| Finding {
            path: path.to_path_buf(),
            line: index + 1,
            reason: format!("stale statement «{}»: {}", statement.text, statement.why),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::removal_findings;

    const REMOVAL: &str = "    manager.removeItemAtURL_error(&url)?;\n";

    #[test]
    fn check_sources_lets_only_the_purge_module_remove_files() {
        assert!(removal_findings(Path::new("crates/rules/src/trash_purge.rs"), REMOVAL).is_empty());
        let elsewhere = removal_findings(Path::new("crates/rules/src/trash_journal.rs"), REMOVAL);
        assert_eq!(elsewhere.len(), 1, "a removal outside the purge module is a finding");
        assert_eq!(elsewhere[0].line, 1);
    }

    #[test]
    fn check_sources_lets_the_mcp_socket_remove_its_own_directory() {
        let cleanup = "        fs::remove_file(&self.path)?;\n        fs::remove_dir(&self.directory)\n";
        assert!(removal_findings(Path::new("crates/agent/src/private_socket.rs"), cleanup).is_empty());
        assert_eq!(
            removal_findings(Path::new("crates/agent/src/claude_code.rs"), cleanup).len(),
            2
        );
    }

    #[test]
    fn check_sources_keeps_tests_free_to_clean_up() {
        assert!(removal_findings(Path::new("crates/rules/tests/trash_purge.rs"), "fs::remove_file(path);").is_empty());
        assert!(
            removal_findings(
                Path::new("crates/app/src/session.rs"),
                "fn work() {}\n#[cfg(test)]\nfn clean() { fs::remove_dir_all(path); }"
            )
            .is_empty()
        );
    }
}
