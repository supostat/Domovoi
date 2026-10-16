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

pub(crate) fn run() -> io::Result<Vec<Finding>> {
    let root = workspace::root();
    let mut findings = Vec::new();
    for statement in &STALE_STATEMENTS {
        let path = root.join(statement.relative_path);
        let content = fs::read_to_string(&path)?;
        findings.extend(occurrences(&path, &content, statement));
    }
    Ok(findings)
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
