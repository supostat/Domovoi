use domovoi_index::{Index, Source};
use domovoi_render::NodeOrigin;
use domovoi_theme::format_size;

use crate::shell_data::ShellData;
use crate::strings::{SOURCE_APFS, SOURCE_MACOS_ESTIMATE, SOURCE_SCAN};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverTip {
    pub name: String,
    pub size_text: String,
    pub path: Option<String>,
    pub source: &'static str,
}

impl HoverTip {
    pub fn for_node(data: &ShellData, index: Option<&Index>, node: usize) -> Option<Self> {
        let record = data.tree.node(node);
        let path = match (record.origin, index) {
            (NodeOrigin::Indexed(id), Some(index)) => Some(index.path(id).display().to_string()),
            _ => None,
        };
        Some(Self {
            name: record.name.clone(),
            size_text: format_size(record.size),
            path,
            source: source_label(record.origin)?,
        })
    }
}

fn source_label(origin: NodeOrigin) -> Option<&'static str> {
    match origin {
        NodeOrigin::Indexed(_) | NodeOrigin::Measured(Source::Scan) => Some(SOURCE_SCAN),
        NodeOrigin::Measured(Source::Getattrlist) => Some(SOURCE_APFS),
        NodeOrigin::Measured(Source::MacOsEstimate) => Some(SOURCE_MACOS_ESTIMATE),
        NodeOrigin::Outline => None,
    }
}
