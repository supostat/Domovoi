use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use domovoi_scanner::{Node, ScanReport};

use crate::app::{App, AppId};
use crate::badges::Badges;
use crate::category::Category;
use crate::copy_groups::CopyGroups;
use crate::enrichment::Enrichment;
use crate::file_types::FileTypes;
use crate::index_node::IndexNode;
use crate::index_root::IndexRoot;
use crate::last_opened_source::LastOpenedSource;
use crate::name_table::{NameId, NameInterner, NameTable};
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::path_rules::{AppDataNaming, RuleProgress};
use crate::reach::Reach;
use crate::timestamp::Timestamp;

const DEPENDENCY_STORE_NAMES: [&str; 2] = ["node_modules", ".pnpm"];
const DEVELOPMENT_DIRECTORY_NAMES: [&str; 9] = [
    "node_modules",
    "zig-cache",
    ".zig-cache",
    "DerivedData",
    ".gradle",
    "Pods",
    "__pycache__",
    ".venv",
    ".build",
];
const PROJECT_MARKERS: [&str; 6] = [
    ".git",
    "Cargo.toml",
    "package.json",
    "go.mod",
    "pyproject.toml",
    "Package.swift",
];

static GENERATIONS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Index {
    pub(crate) nodes: Vec<IndexNode>,
    pub(crate) names: NameTable,
    pub(crate) root: IndexRoot,
    pub(crate) apps: Vec<App>,
    #[rkyv(with = rkyv::with::Skip)]
    pub(crate) generation: u64,
    #[rkyv(with = rkyv::with::Skip)]
    pub(crate) copy_groups: CopyGroups,
    #[rkyv(with = rkyv::with::Skip)]
    pub(crate) hidden_folders: u64,
    #[rkyv(with = rkyv::with::Skip)]
    pub(crate) garbage: usize,
}

impl PartialEq for Index {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes && self.names == other.names && self.root == other.root && self.apps == other.apps
    }
}

impl Eq for Index {}

impl Index {
    pub fn build(report: &ScanReport, root: IndexRoot, enrichment: &Enrichment) -> Self {
        let mut interner = NameInterner::default();
        let mut nodes = Vec::new();
        append_subtree(&mut nodes, &mut interner, &report.root, NodeId::NONE);
        let mut index = Self {
            nodes,
            names: interner.finish(),
            root,
            apps: Vec::new(),
            generation: next_generation(),
            copy_groups: CopyGroups::default(),
            hidden_folders: 0,
            garbage: 0,
        };
        index.derive(enrichment);
        index
    }

    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    pub fn len(&self) -> usize {
        self.nodes.len() - self.garbage
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_live(&self, id: NodeId) -> bool {
        if id.index() >= self.nodes.len() {
            return false;
        }
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            let first = self.node(parent).first_child.0;
            if !(first..first + self.node(parent).child_count).contains(&current.0) {
                return false;
            }
            current = parent;
        }
        current == NodeId::ROOT
    }

    pub fn node(&self, id: NodeId) -> &IndexNode {
        &self.nodes[id.index()]
    }

    pub fn name(&self, id: NodeId) -> &OsStr {
        self.names.get(self.node(id).name)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.node(id).parent;
        (parent != NodeId::NONE).then_some(parent)
    }

    pub fn children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let node = self.node(id);
        let first = node.first_child.0;
        (first..first + node.child_count).map(NodeId)
    }

    pub fn child_named(&self, id: NodeId, name: &OsStr) -> Option<NodeId> {
        let node = self.node(id);
        let first = node.first_child.index();
        let siblings = &self.nodes[first..first + node.child_count as usize];
        siblings
            .binary_search_by(|sibling| self.names.get(sibling.name).as_bytes().cmp(name.as_bytes()))
            .ok()
            .map(|offset| NodeId::from_index(first + offset))
    }

    pub fn path(&self, id: NodeId) -> PathBuf {
        self.root.canonical(&self.relative_path(id))
    }

    pub fn physical_path(&self, id: NodeId) -> PathBuf {
        self.root.scan_path().join(self.relative_path(id))
    }

    pub fn find(&self, canonical: &Path) -> Option<NodeId> {
        let relative = self.root.relative(canonical)?;
        relative.components().try_fold(NodeId::ROOT, |node, component| {
            self.child_named(node, component.as_os_str())
        })
    }

    pub fn app(&self, id: AppId) -> Option<&App> {
        self.apps.get(id.0 as usize)
    }

    pub fn apps(&self) -> &[App] {
        &self.apps
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn unique_inode_allocation(&self) -> u64 {
        let mut by_inode = HashMap::new();
        for id in self.subtree(self.root()) {
            let node = self.node(id);
            if node.kind != NodeKind::Unknown {
                by_inode.insert((node.real_dev, node.file_id), node.alloc_size);
            }
        }
        by_inode.values().sum()
    }

    pub fn hidden_folders(&self) -> u64 {
        self.hidden_folders
    }

    pub(crate) fn count_hidden(&self, ids: &[NodeId]) -> u64 {
        ids.iter().filter(|id| self.node(**id).reach.hides_content()).count() as u64
    }

    pub fn depth_of(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            depth += 1;
            current = parent;
        }
        depth
    }

    pub(crate) fn relative_path(&self, id: NodeId) -> PathBuf {
        let mut names = Vec::new();
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            names.push(self.name(current));
            current = parent;
        }
        names.iter().rev().collect()
    }

    pub(crate) fn subtree(&self, id: NodeId) -> Vec<NodeId> {
        let mut pending = vec![id];
        let mut subtree = Vec::new();
        while let Some(current) = pending.pop() {
            subtree.push(current);
            pending.extend(self.children(current));
        }
        subtree
    }

    pub(crate) fn derive(&mut self, enrichment: &Enrichment) {
        let live = self.subtree(self.root());
        self.apply_spotlight_dates(enrichment);
        self.assign_categories_and_owners(&live, enrichment);
        self.mark_models(enrichment);
        let counted = self.choose_canonical_copies(&live);
        self.aggregate(&live, &counted);
        self.hidden_folders = self.count_hidden(&live);
    }

    pub fn compact_garbage(&mut self) -> bool {
        if self.garbage == 0 {
            return false;
        }
        self.compact();
        let live = self.subtree(self.root());
        self.copy_groups = self.collect_copy_groups(&live);
        self.generation = next_generation();
        true
    }

    pub(crate) fn advance_generation(&mut self) {
        self.generation = next_generation();
    }

    pub(crate) fn restore_derived_state(&mut self) {
        self.generation = next_generation();
        let live = self.subtree(self.root());
        self.garbage = self.nodes.len() - live.len();
        self.hidden_folders = self.count_hidden(&live);
        self.copy_groups = self.collect_copy_groups(&live);
    }

    pub(crate) fn compact(&mut self) {
        let mut interner = NameInterner::default();
        let mut nodes = Vec::with_capacity(self.nodes.len());
        let mut root = self.nodes[0].clone();
        root.name = interner.intern(self.names.get(root.name));
        nodes.push(root);
        let mut pending = vec![(NodeId::ROOT, NodeId::ROOT)];
        while let Some((old, new)) = pending.pop() {
            let first = NodeId::from_index(nodes.len());
            let children: Vec<NodeId> = self.children(old).collect();
            for child in &children {
                let mut record = self.nodes[child.index()].clone();
                record.name = interner.intern(self.names.get(record.name));
                record.parent = new;
                nodes.push(record);
            }
            let new_node = &mut nodes[new.index()];
            new_node.first_child = first;
            new_node.child_count = u32::try_from(children.len()).expect("child count fits in u32");
            for (offset, child) in children.into_iter().enumerate() {
                pending.push((child, NodeId::from_index(first.index() + offset)));
            }
        }
        self.nodes = nodes;
        self.names = interner.finish();
        self.garbage = 0;
    }

    pub(crate) fn apply_spotlight_dates(&mut self, enrichment: &Enrichment) {
        for (path, date) in &enrichment.spotlight_last_used {
            let Some(id) = self.find(path) else {
                continue;
            };
            let node = &mut self.nodes[id.index()];
            if node.kind == NodeKind::Directory {
                node.last_opened = Timestamp::from_system_time(*date);
                node.last_opened_source = LastOpenedSource::SpotlightIndex;
            }
        }
    }

    fn assign_categories_and_owners(&mut self, live: &[NodeId], enrichment: &Enrichment) {
        let count = self.nodes.len();
        let mut progress = vec![RuleProgress::default(); count];
        let mut depth = vec![0_usize; count];
        let mut file_types = FileTypes::default();
        self.apps.clear();
        let (root_progress, root_category) = self.root_rules();
        progress[0] = root_progress;
        depth[0] = self.root.root_components().len();
        self.nodes[0].category = root_category;
        self.nodes[0].app = AppId::NONE;
        for id in &live[1..] {
            let index = id.index();
            let parent = self.nodes[index].parent.index();
            progress[index] = self.categorize(*id, progress[parent], depth[parent], &mut file_types, enrichment);
            depth[index] = depth[parent] + 1;
        }
    }

    pub(crate) fn categorize(
        &mut self,
        id: NodeId,
        parent_progress: RuleProgress,
        parent_depth: usize,
        file_types: &mut FileTypes,
        enrichment: &Enrichment,
    ) -> RuleProgress {
        let index = id.index();
        let parent = self.nodes[index].parent.index();
        let name = self.names.get(self.nodes[index].name).to_owned();
        let depth = parent_depth + 1;
        let progress = parent_progress.descend(depth, &name);
        let category = self.category_of(index, parent, &name, progress, depth, file_types);
        self.nodes[index].category = category;
        let app_data = parent_progress
            .completed(parent_depth)
            .and_then(|parent_rule| parent_rule.app_data);
        self.nodes[index].app = match app_data.and_then(|naming| bundle_id(&name, naming)) {
            Some(bundle_id) => self.owner_of(bundle_id, enrichment),
            None => self.nodes[parent].app,
        };
        progress
    }

    pub(crate) fn rule_progress_of(&self, id: NodeId) -> (RuleProgress, usize) {
        let mut chain = Vec::new();
        let mut current = id;
        while let Some(parent) = self.parent(current) {
            chain.push(current);
            current = parent;
        }
        let (mut progress, _) = self.root_rules();
        let mut depth = self.root.root_components().len();
        for member in chain.into_iter().rev() {
            depth += 1;
            progress = progress.descend(depth, self.name(member));
        }
        (progress, depth)
    }

    fn owner_of(&mut self, bundle_id: String, enrichment: &Enrichment) -> AppId {
        if let Some(position) = self.apps.iter().position(|app| app.bundle_id == bundle_id) {
            return AppId(u32::try_from(position).expect("app count fits in u32"));
        }
        let installation = enrichment.applications.resolve(&bundle_id);
        let last_opened = application_last_opened(&installation, enrichment);
        self.apps.push(App {
            bundle_id,
            installation,
            last_opened,
        });
        AppId(u32::try_from(self.apps.len() - 1).expect("app count fits in u32"))
    }

    pub(crate) fn root_rules(&self) -> (RuleProgress, Category) {
        let mut progress = RuleProgress::start();
        let mut category = Category::Home;
        for (position, component) in self.root.root_components().iter().enumerate() {
            progress = progress.descend(position + 1, OsStr::new(component));
            if let Some(path_rule) = progress.completed(position + 1) {
                category = path_rule.category;
            }
        }
        (progress, category)
    }

    fn category_of(
        &self,
        index: usize,
        parent: usize,
        name: &OsStr,
        progress: RuleProgress,
        depth: usize,
        file_types: &mut FileTypes,
    ) -> Category {
        let parent_category = self.nodes[parent].category;
        if let Some(path_rule) = progress.completed(depth) {
            return path_rule.category;
        }
        if parent == 0 && self.root.is_service_entry(name.as_bytes()) {
            return Category::System;
        }
        if !matches!(parent_category, Category::Home | Category::Docs) {
            return parent_category;
        }
        let is_directory = self.nodes[index].kind == NodeKind::Directory;
        if is_directory && self.is_development_directory(NodeId::from_index(index), name) {
            return Category::Dev;
        }
        match file_types.category(name, is_directory) {
            Some(Category::Apps) => Category::Apps,
            Some(category) if parent_category == Category::Home => category,
            _ => parent_category,
        }
    }

    fn is_development_directory(&self, id: NodeId, name: &OsStr) -> bool {
        DEVELOPMENT_DIRECTORY_NAMES
            .iter()
            .any(|known| OsStr::new(known) == name)
            || PROJECT_MARKERS
                .iter()
                .any(|marker| self.child_named(id, OsStr::new(marker)).is_some())
    }

    pub(crate) fn mark_models(&mut self, enrichment: &Enrichment) {
        for path in &enrichment.model_paths {
            if let Some(id) = self.find(path) {
                for member in self.subtree(id) {
                    self.nodes[member.index()].category = Category::Model;
                }
            }
        }
    }

    pub fn inside_dependency_store(&self, id: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(node) = current {
            let name = self.name(node);
            if DEPENDENCY_STORE_NAMES.iter().any(|store| OsStr::new(store) == name) {
                return true;
            }
            current = self.parent(node);
        }
        false
    }

    fn aggregate(&mut self, live: &[NodeId], counted: &[bool]) {
        for id in live {
            self.reset_own_totals(*id, counted[id.index()]);
        }
        for id in live[1..].iter().rev() {
            let child = self.nodes[id.index()].clone();
            add_child_totals(&mut self.nodes[child.parent.index()], &child);
        }
    }

    pub(crate) fn reset_own_totals(&mut self, id: NodeId, counted: bool) {
        let node = &mut self.nodes[id.index()];
        let is_directory = node.kind == NodeKind::Directory;
        node.allocated = if counted { node.alloc_size } else { 0 };
        node.files = u32::from(!is_directory && node.kind != NodeKind::Unknown);
        node.newest_modified = if is_directory {
            Timestamp::UNKNOWN
        } else {
            node.modified
        };
        node.newest_opened = node.last_opened;
        node.flags_in_subtree = node.flags;
    }
}

fn next_generation() -> u64 {
    GENERATIONS.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn add_child_totals(parent: &mut IndexNode, child: &IndexNode) {
    parent.allocated += child.allocated;
    parent.files += child.files;
    parent.newest_modified = parent.newest_modified.max(child.newest_modified);
    parent.newest_opened = parent.newest_opened.max(child.newest_opened);
    parent.flags_in_subtree |= child.flags_in_subtree;
}

pub(crate) fn totals(node: &IndexNode) -> (u64, u32, Timestamp, Timestamp, u32) {
    (
        node.allocated,
        node.files,
        node.newest_modified,
        node.newest_opened,
        node.flags_in_subtree,
    )
}

fn bundle_id(name: &OsStr, naming: AppDataNaming) -> Option<String> {
    let name = name.to_str()?;
    let candidate = match naming {
        AppDataNaming::BundleId => name,
        AppDataNaming::Suffixed(suffix) => name.strip_suffix(suffix)?,
    };
    let looks_like_bundle_id = candidate.split('.').count() >= 2
        && candidate.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
        });
    looks_like_bundle_id.then(|| candidate.to_owned())
}

fn application_last_opened(installation: &crate::app::Installation, enrichment: &Enrichment) -> Timestamp {
    let crate::app::Installation::Installed { bundle_path } = installation else {
        return Timestamp::UNKNOWN;
    };
    enrichment
        .spotlight_last_used
        .iter()
        .find(|(path, _)| path == Path::new(bundle_path))
        .map_or(Timestamp::UNKNOWN, |(_, date)| Timestamp::from_system_time(*date))
}

fn append_subtree(nodes: &mut Vec<IndexNode>, interner: &mut NameInterner, source: &Node, parent: NodeId) -> NodeId {
    let id = NodeId::from_index(nodes.len());
    nodes.push(record(source, parent, interner.intern(&source.name)));
    append_children(nodes, interner, id, source);
    id
}

pub(crate) fn append_children(nodes: &mut Vec<IndexNode>, interner: &mut NameInterner, id: NodeId, source: &Node) {
    let mut pending = vec![(id, source)];
    while let Some((parent, parent_source)) = pending.pop() {
        let mut children: Vec<&Node> = parent_source.children.iter().collect();
        children.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
        let first = NodeId::from_index(nodes.len());
        for child in &children {
            nodes.push(record(child, parent, interner.intern(&child.name)));
        }
        let parent_node = &mut nodes[parent.index()];
        parent_node.first_child = first;
        parent_node.child_count = u32::try_from(children.len()).expect("child count fits in u32");
        for (offset, child) in children.into_iter().enumerate() {
            pending.push((NodeId::from_index(first.index() + offset), child));
        }
    }
}

pub(crate) fn record(source: &Node, parent: NodeId, name: NameId) -> IndexNode {
    let attributes = source.attributes.as_ref();
    let created = attributes.map_or(Timestamp::UNKNOWN, |attributes| {
        Timestamp::from_system_time(attributes.created)
    });
    let added = attributes
        .and_then(|attributes| attributes.added)
        .map_or(Timestamp::UNKNOWN, Timestamp::from_system_time);
    let (last_opened, last_opened_source) = match source.last_used {
        Some(time) => {
            let opened = Timestamp::from_system_time(time);
            let source = if opened < added.max(created) {
                LastOpenedSource::Inherited
            } else {
                LastOpenedSource::Xattr
            };
            (opened, source)
        }
        None => (Timestamp::UNKNOWN, LastOpenedSource::Unknown),
    };
    IndexNode {
        name,
        parent,
        first_child: NodeId::NONE,
        child_count: 0,
        kind: NodeKind::of(attributes.map(|attributes| attributes.kind)),
        reach: Reach::from(source.state),
        category: Category::Home,
        badges: Badges::default(),
        last_opened_source,
        flags: attributes.map_or(0, |attributes| attributes.flags),
        real_dev: attributes.and_then(|attributes| attributes.real_dev).unwrap_or(0),
        link_count: attributes.and_then(|attributes| attributes.link_count).unwrap_or(1),
        clone_refcount: attributes.and_then(|attributes| attributes.clone_refcount).unwrap_or(0),
        shares_with: 0,
        files: 0,
        flags_in_subtree: 0,
        app: AppId::NONE,
        file_id: attributes.map_or(0, |attributes| attributes.file_id),
        link_id: attributes.and_then(|attributes| attributes.link_id).unwrap_or(0),
        clone_id: attributes.and_then(|attributes| attributes.clone_id).unwrap_or(0),
        ext_flags: attributes.and_then(|attributes| attributes.ext_flags).unwrap_or(0),
        alloc_size: attributes.and_then(|attributes| attributes.alloc_size).unwrap_or(0),
        total_size: attributes.and_then(|attributes| attributes.total_size).unwrap_or(0),
        allocated: 0,
        modified: attributes.map_or(Timestamp::UNKNOWN, |attributes| {
            Timestamp::from_system_time(attributes.modified)
        }),
        created,
        added,
        last_opened,
        newest_modified: Timestamp::UNKNOWN,
        newest_opened: Timestamp::UNKNOWN,
    }
}
