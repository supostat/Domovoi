use domovoi_index::{Category, Index, Source};
use domovoi_render::{DiagramNode, NodeOrigin};
use domovoi_theme::{format_count, format_size};

use crate::shell_data::ShellData;
use crate::strings::{
    DIAGRAM, OTHER_USER_HOMES, OTHER_USER_HOMES_HINT, PURGEABLE, PURGEABLE_PLAIN, PURGEABLE_TIME_MACHINE,
    SNAPSHOTS_RELEASE, SOURCE_APFS, SOURCE_MACOS_ESTIMATE, SOURCE_SCAN, UNACCOUNTED, UNACCOUNTED_HINT,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverTip {
    pub name: String,
    pub size_text: String,
    pub path: Option<String>,
    pub source: &'static str,
    pub note: Option<String>,
}

impl HoverTip {
    pub fn for_node(data: &ShellData, index: Option<&Index>, node: usize) -> Option<Self> {
        let record = data.tree.node(node);
        let path = match (record.origin, index) {
            (NodeOrigin::Indexed(id), Some(index)) => Some(index.path(id).display().to_string()),
            _ => None,
        };
        Some(Self {
            name: data.tree.display_name(node, DIAGRAM.nested_separator),
            size_text: format_size(record.size),
            path,
            source: source_label(record.origin)?,
            note: hidden_note(data, record),
        })
    }
}

fn hidden_note(data: &ShellData, record: &DiagramNode) -> Option<String> {
    if !matches!(record.origin, NodeOrigin::Measured(_)) || record.category != Category::Hidden {
        return None;
    }
    if record.name == PURGEABLE {
        let note = if data.time_machine.explains_purgeable() {
            PURGEABLE_TIME_MACHINE
        } else {
            PURGEABLE_PLAIN
        };
        return Some(note.to_owned());
    }
    if record.name == UNACCOUNTED {
        if data.other_user_homes == 0 {
            return Some(UNACCOUNTED_HINT.to_owned());
        }
        return Some(format!(
            "{UNACCOUNTED_HINT} {}: {OTHER_USER_HOMES_HINT}.",
            format_count(data.other_user_homes, &OTHER_USER_HOMES)
        ));
    }
    data.snapshots_present.then(|| SNAPSHOTS_RELEASE.to_owned())
}

fn source_label(origin: NodeOrigin) -> Option<&'static str> {
    match origin {
        NodeOrigin::Indexed(_) | NodeOrigin::Measured(Source::Scan) => Some(SOURCE_SCAN),
        NodeOrigin::Measured(Source::Getattrlist) => Some(SOURCE_APFS),
        NodeOrigin::Measured(Source::MacOsEstimate) => Some(SOURCE_MACOS_ESTIMATE),
        NodeOrigin::Outline => None,
    }
}
