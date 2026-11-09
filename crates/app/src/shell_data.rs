use std::collections::{BTreeMap, HashMap};
use std::iter;

use domovoi_index::{Category, Index, NodeId, TimeMachineSettings};
use domovoi_render::{DiagramTree, MarkedNodes, NodeOrigin, ROOT};
use domovoi_rules::Tray;
use domovoi_theme::{format_count, format_size};

use crate::access_state::AccessState;
use crate::ai_models::AiModels;
use crate::care_models::CareModels;
use crate::crumb::Crumb;
use crate::disk_overview::{DiskOverview, in_cloud_bytes};
use crate::expansion::Expansion;
use crate::legend_entry::LegendEntry;
use crate::level_row::LevelRow;
use crate::navigation::Navigation;
use crate::scan_status::ScanStatus;
use crate::section::Section;
use crate::sidebar_entry::SidebarEntry;
use crate::strings::{
    ASSISTANT, ASSISTANT_BADGE, DIAGRAM, DISK_GROUP, DUPLICATES, FREE, HIDDEN_BY_MACOS, HIDDEN_FOLDERS,
    HIDDEN_FOLDERS_HINT, MODEL, OF, ORDER_GROUP, OTHER_USER_HOMES, OTHER_USER_HOMES_HINT, RECOMMENDATIONS,
    SETTINGS_GROUP,
};

#[derive(Clone, Debug)]
pub struct ShellData {
    pub disk_name: String,
    pub access: AccessState,
    pub tree: DiagramTree,
    pub disk_total_bytes: u64,
    pub free_bytes: u64,
    pub in_cloud_bytes: u64,
    pub scan: ScanStatus,
    pub care: CareModels,
    pub ai: AiModels,
    pub care_ready: bool,
    pub rescan_allowed: bool,
    pub hidden_folders: u64,
    pub snapshots_present: bool,
    pub other_user_homes: u64,
    pub time_machine: TimeMachineSettings,
    pub marked: MarkedNodes,
}

impl ShellData {
    pub fn measured(
        access: AccessState,
        overview: &DiskOverview,
        index: Option<&Index>,
        expansion: &Expansion,
        scan: ScanStatus,
        care: CareModels,
        ai: AiModels,
    ) -> Self {
        Self {
            disk_name: overview.name.clone(),
            access,
            tree: DiagramTree::from_outline(&overview.outline(index, expansion)),
            disk_total_bytes: overview.disk_total_bytes(),
            free_bytes: overview.free_bytes(),
            in_cloud_bytes: index.map_or(0, in_cloud_bytes),
            scan,
            care,
            ai,
            care_ready: index.is_some(),
            rescan_allowed: false,
            hidden_folders: index.map_or(0, Index::hidden_folders),
            snapshots_present: !overview.volumes.snapshots.is_empty(),
            other_user_homes: index.map_or(0, Index::other_user_homes),
            time_machine: overview.time_machine,
            marked: MarkedNodes::default(),
        }
    }

    #[must_use]
    pub(crate) fn carrying_index_of(self, previous: Self) -> Self {
        Self {
            tree: previous.tree,
            in_cloud_bytes: previous.in_cloud_bytes,
            care: previous.care,
            care_ready: previous.care_ready,
            hidden_folders: previous.hidden_folders,
            other_user_homes: previous.other_user_homes,
            marked: previous.marked,
            ..self
        }
    }

    pub fn mark_tray(&mut self, index: &Index, tray: &Tray) {
        let positions: HashMap<NodeId, usize> = (0..self.tree.len())
            .filter_map(|position| match self.tree.node(position).origin {
                NodeOrigin::Indexed(node) => Some((node, position)),
                _ => None,
            })
            .collect();
        let mut marked = MarkedNodes::default();
        for item in tray.items() {
            if let Some(position) = positions.get(&item.node) {
                marked.mark(*position);
                continue;
            }
            let nearest_shown = iter::successors(index.parent(item.node), |node| index.parent(*node))
                .find_map(|ancestor| positions.get(&ancestor).copied());
            if let Some(position) = nearest_shown {
                marked.deduct(position, index.node(item.node).allocated);
            }
        }
        self.marked = marked;
    }

    pub fn tree_position(&self, origin: NodeOrigin) -> Option<usize> {
        (0..self.tree.len()).find(|node| self.tree.node(*node).origin == origin)
    }

    pub fn used_bytes(&self) -> u64 {
        self.tree.node(ROOT).size
    }

    pub fn used_share(&self) -> f64 {
        self.used_bytes() as f64 / self.disk_total_bytes.max(1) as f64
    }

    pub fn free_line(&self) -> String {
        format!(
            "{FREE} {} {OF} {}",
            format_size(self.free_bytes),
            format_size(self.disk_total_bytes)
        )
    }

    pub fn hidden_notice(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.hidden_folders > 0 {
            parts.push(format!(
                "{HIDDEN_BY_MACOS} {}. {HIDDEN_FOLDERS_HINT}",
                format_count(self.hidden_folders, &HIDDEN_FOLDERS)
            ));
        }
        if self.other_user_homes > 0 {
            parts.push(format!(
                "{}: {OTHER_USER_HOMES_HINT}",
                format_count(self.other_user_homes, &OTHER_USER_HOMES)
            ));
        }
        (!parts.is_empty()).then(|| parts.join(". "))
    }

    pub fn free_short(&self) -> String {
        format!("{FREE} {}", format_size(self.free_bytes))
    }

    pub fn sidebar(&self, navigation: Navigation) -> Vec<SidebarEntry> {
        let entry = |section: Section, title: &str, counter: Option<String>| SidebarEntry {
            title: title.to_owned(),
            counter,
            children: Vec::new(),
            section: navigation.allows(section).then_some(section),
            selected: section == navigation.current,
        };
        let group = |title: &str, children| SidebarEntry {
            title: title.to_owned(),
            counter: None,
            children,
            section: None,
            selected: false,
        };
        let recommendations = self.care.recommendations.total();
        let duplicates = self.care.duplicates.can_free();
        vec![
            group(
                DISK_GROUP,
                vec![entry(
                    Section::Disk,
                    &self.disk_name,
                    Some(format_size(self.used_bytes())),
                )],
            ),
            group(
                ORDER_GROUP,
                vec![
                    entry(
                        Section::Recommendations,
                        RECOMMENDATIONS,
                        (recommendations > 0).then(|| format_size(recommendations)),
                    ),
                    entry(
                        Section::Duplicates,
                        DUPLICATES,
                        (duplicates > 0).then(|| format_size(duplicates)),
                    ),
                    entry(Section::Assistant, ASSISTANT, Some(ASSISTANT_BADGE.to_owned())),
                ],
            ),
            group(SETTINGS_GROUP, vec![entry(Section::Model, MODEL, None)]),
        ]
    }

    pub fn level_rows(&self, focus: usize) -> Vec<LevelRow> {
        let children = &self.tree.node(focus).children;
        let largest = children.first().map_or(1, |child| self.tree.node(*child).size.max(1));
        children
            .iter()
            .map(|child| {
                let node = self.tree.node(*child);
                LevelRow {
                    node: *child,
                    name: self.tree.display_name(*child, DIAGRAM.nested_separator),
                    size_text: format_size(node.size),
                    share: node.size as f64 / largest as f64,
                    category: node.category,
                    draggable: matches!(node.origin, NodeOrigin::Indexed(_)),
                    marked: self.marked.covers(&self.tree, *child),
                }
            })
            .collect()
    }

    pub fn legend(&self) -> Vec<LegendEntry> {
        let mut totals: BTreeMap<usize, u64> = BTreeMap::new();
        let mut pending = vec![ROOT];
        while let Some(node) = pending.pop() {
            let record = self.tree.node(node);
            if record.children.is_empty() {
                let position = Category::ALL
                    .iter()
                    .position(|category| *category == record.category)
                    .expect("known category");
                *totals.entry(position).or_default() += record.size;
            }
            pending.extend(record.children.iter().copied());
        }
        let mut entries: Vec<(u64, LegendEntry)> = totals
            .into_iter()
            .map(|(position, bytes)| {
                let category = Category::ALL[position];
                (
                    bytes,
                    LegendEntry {
                        category,
                        title: DIAGRAM.category_name(category).to_owned(),
                        size_text: format_size(bytes),
                    },
                )
            })
            .collect();
        entries.sort_by_key(|(bytes, _)| std::cmp::Reverse(*bytes));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }

    pub fn crumbs(&self, focus: usize) -> Vec<Crumb> {
        self.tree
            .path(focus)
            .into_iter()
            .map(|node| Crumb {
                node,
                title: self.tree.node(node).name.clone(),
            })
            .collect()
    }
}
