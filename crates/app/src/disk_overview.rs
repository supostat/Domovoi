use std::io;
use std::path::Path;

use domovoi_index::{Category, HiddenSpace, Index, Measured, Reach, Volumes};
use domovoi_render::{DiagramOutline, NodeOrigin};

use crate::expansion::Expansion;
use crate::strings::{DATA_VOLUME, DIAGRAM, MACOS, OTHER_VOLUMES, PURGEABLE, SYSTEM_DATA};
use crate::volume_name::volume_name;

pub(crate) const SYSTEM_DATA_ROOT: &str = "/System/Volumes/Data";
const SYSTEM_ROOT: &str = "/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskOverview {
    pub name: String,
    pub volumes: Volumes,
}

impl DiskOverview {
    pub fn measure(data_root: &Path) -> io::Result<Self> {
        let named = if data_root == Path::new(SYSTEM_DATA_ROOT) {
            Path::new(SYSTEM_ROOT)
        } else {
            data_root
        };
        Ok(Self {
            name: volume_name(named)?,
            volumes: Volumes::measure(data_root)?,
        })
    }

    pub fn outline(&self, index: Option<&Index>, expansion: &Expansion) -> DiagramOutline {
        let mut children = match index {
            Some(index) => {
                let data = DiagramOutline::indexed(index, index.root(), &|node, _| expansion.expands(node));
                let mut children = data.children;
                children.extend(hidden_outline(&self.volumes.hidden(index)));
                children
            }
            None => vec![measured(DATA_VOLUME, Category::Home, self.volumes.data_used)],
        };
        children.push(measured(OTHER_VOLUMES, Category::System, self.volumes.other_volumes));
        children.push(measured(MACOS, Category::System, self.volumes.system_used));
        children.retain(|child| child.size > 0 || !child.children.is_empty());
        DiagramOutline {
            name: self.name.clone(),
            category: Category::Home,
            size: 0,
            origin: NodeOrigin::Measured(self.volumes.container_size.source),
            children,
        }
    }

    pub fn disk_total_bytes(&self) -> u64 {
        self.volumes.container_size.value
    }

    pub fn free_bytes(&self) -> u64 {
        self.volumes.free.value
    }
}

pub fn in_cloud_bytes(index: &Index) -> u64 {
    let mut total = 0;
    let mut pending = vec![index.root()];
    while let Some(node) = pending.pop() {
        let record = index.node(node);
        if record.reach == Reach::InCloud {
            total += record.total_size;
        }
        pending.extend(index.children(node));
    }
    total
}

fn hidden_outline(hidden: &HiddenSpace) -> Option<DiagramOutline> {
    let total = u64::try_from(hidden.total.value).ok().filter(|total| *total > 0)?;
    let overhead = u64::try_from(hidden.overhead).unwrap_or(0);
    let purgeable = hidden.purgeable.value.min(total);
    let children = if overhead > 0 && purgeable > 0 {
        vec![
            measured(PURGEABLE, Category::Hidden, hidden.purgeable),
            measured(
                SYSTEM_DATA,
                Category::Hidden,
                Measured {
                    value: overhead,
                    source: hidden.total.source,
                },
            ),
        ]
    } else {
        Vec::new()
    };
    Some(DiagramOutline {
        name: DIAGRAM.category_name(Category::Hidden).to_owned(),
        category: Category::Hidden,
        size: total,
        origin: NodeOrigin::Measured(hidden.total.source),
        children,
    })
}

fn measured(name: &str, category: Category, value: Measured<u64>) -> DiagramOutline {
    DiagramOutline {
        name: name.to_owned(),
        category,
        size: value.value,
        origin: NodeOrigin::Measured(value.source),
        children: Vec::new(),
    }
}
