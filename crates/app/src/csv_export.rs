use std::fmt::Write;

use domovoi_index::{Index, NodeId};
use domovoi_theme::format_iso_utc;

const HEADER: &str = "path,bytes,category,modified";
const LINE_END: &str = "\r\n";
const QUOTE: char = '"';

pub fn csv_lines(index: &Index, root: NodeId) -> String {
    let mut lines = String::new();
    lines.push_str(HEADER);
    lines.push_str(LINE_END);
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let record = index.node(id);
        let modified = record.modified.system_time().map(format_iso_utc).unwrap_or_default();
        write!(
            lines,
            "{},{},{},{}{LINE_END}",
            escaped(&index.path(id).to_string_lossy()),
            record.allocated,
            record.category.id(),
            modified
        )
        .expect("writing into a string never fails");
        let children: Vec<NodeId> = index.children(id).collect();
        pending.extend(children.into_iter().rev());
    }
    lines
}

fn escaped(field: &str) -> String {
    if field.contains([',', QUOTE, '\n', '\r']) {
        let doubled = field.replace(QUOTE, "\"\"");
        format!("{QUOTE}{doubled}{QUOTE}")
    } else {
        field.to_owned()
    }
}
