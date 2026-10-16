use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::sys::{EF_MAY_SHARE_BLOCKS, EF_SHARES_ALL_BLOCKS, UF_COMPRESSED};
use domovoi_scanner::{Node, ScanReport};

use crate::app::{App, AppId};
use crate::badges::Badges;
use crate::category::Category;
use crate::enrichment::Enrichment;
use crate::file_types::FileTypes;
use crate::index_node::IndexNode;
use crate::index_root::IndexRoot;
use crate::last_opened_source::LastOpenedSource;
use crate::name_table::{NameInterner, NameTable};
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

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Index {
    pub(crate) nodes: Vec<IndexNode>,
    pub(crate) names: NameTable,
    pub(crate) root: IndexRoot,
    pub(crate) apps: Vec<App>,
}

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
        };
        index.derive(enrichment);
        index
    }

    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
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

    pub fn unique_inode_allocation(&self) -> u64 {
        let mut by_inode = HashMap::new();
        for node in &self.nodes {
            if node.kind != NodeKind::Unknown {
                by_inode.insert((node.real_dev, node.file_id), node.alloc_size);
            }
        }
        by_inode.values().sum()
    }

    pub fn hidden_folders(&self) -> u64 {
        self.nodes.iter().filter(|node| node.reach.hides_content()).count() as u64
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
        self.apply_spotlight_dates(enrichment);
        self.assign_categories_and_owners(enrichment);
        self.mark_models(enrichment);
        let counted = self.choose_canonical_copies();
        self.aggregate(&counted);
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
    }

    pub(crate) fn replace_children(&mut self, id: NodeId, source: &Node) {
        let mut interner = NameInterner::from_table(std::mem::take(&mut self.names));
        let mut nodes = std::mem::take(&mut self.nodes);
        let reach = Reach::from(source.state);
        append_children(&mut nodes, &mut interner, id, source);
        nodes[id.index()].reach = reach;
        self.nodes = nodes;
        self.names = interner.finish();
        self.compact();
    }

    fn apply_spotlight_dates(&mut self, enrichment: &Enrichment) {
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

    fn assign_categories_and_owners(&mut self, enrichment: &Enrichment) {
        let count = self.nodes.len();
        let mut progress = vec![RuleProgress::default(); count];
        let mut depth = vec![0_usize; count];
        let mut file_types = FileTypes::default();
        let mut app_ids: HashMap<String, AppId> = HashMap::new();
        self.apps.clear();
        let (root_progress, root_category) = self.root_rules();
        progress[0] = root_progress;
        depth[0] = self.root.root_components().len();
        self.nodes[0].category = root_category;
        self.nodes[0].app = AppId::NONE;
        for index in 1..count {
            let parent = self.nodes[index].parent.index();
            let name = self.names.get(self.nodes[index].name).to_owned();
            depth[index] = depth[parent] + 1;
            progress[index] = progress[parent].descend(depth[index], &name);
            let category = self.category_of(index, parent, &name, progress[index], depth[index], &mut file_types);
            self.nodes[index].category = category;
            let app_data = progress[parent]
                .completed(depth[parent])
                .and_then(|parent_rule| parent_rule.app_data);
            self.nodes[index].app = match app_data.and_then(|naming| bundle_id(&name, naming)) {
                Some(bundle_id) => {
                    let next = AppId(u32::try_from(self.apps.len()).expect("app count fits in u32"));
                    *app_ids.entry(bundle_id.clone()).or_insert_with(|| {
                        let installation = enrichment.applications.resolve(&bundle_id);
                        let last_opened = application_last_opened(&installation, enrichment);
                        self.apps.push(App {
                            bundle_id,
                            installation,
                            last_opened,
                        });
                        next
                    })
                }
                None => self.nodes[parent].app,
            };
        }
    }

    fn root_rules(&self) -> (RuleProgress, Category) {
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

    fn mark_models(&mut self, enrichment: &Enrichment) {
        for path in &enrichment.model_paths {
            if let Some(id) = self.find(path) {
                for member in self.subtree(id) {
                    self.nodes[member.index()].category = Category::Model;
                }
            }
        }
    }

    fn choose_canonical_copies(&mut self) -> Vec<bool> {
        let mut counted: Vec<bool> = self.nodes.iter().map(|node| node.kind != NodeKind::Unknown).collect();
        for node in &mut self.nodes {
            node.shares_with = 0;
            node.badges.remove(Badges::HARD_LINK);
            node.badges.remove(Badges::CLONE);
            node.badges.remove(Badges::CLONE_CANONICAL);
            if node.flags & UF_COMPRESSED != 0 {
                node.badges.insert(Badges::COMPRESSED);
            }
        }
        let mut entries_by_inode: HashMap<(u32, u64), Vec<NodeId>> = HashMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if node.link_count > 1 && node.kind != NodeKind::Directory {
                entries_by_inode
                    .entry((node.real_dev, node.file_id))
                    .or_default()
                    .push(NodeId::from_index(index));
            }
        }
        for entries in entries_by_inode.values().filter(|entries| entries.len() > 1) {
            let canonical = *entries
                .iter()
                .min_by_key(|entry| self.relative_path(**entry))
                .expect("a hard-link group is not empty");
            for entry in entries {
                self.nodes[entry.index()].badges.insert(Badges::HARD_LINK);
                counted[entry.index()] = *entry == canonical;
            }
        }
        let mut members_by_stream: HashMap<(u32, u64), Vec<NodeId>> = HashMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if counted[index] && node.kind == NodeKind::File && may_share_blocks(node) {
                members_by_stream
                    .entry((node.real_dev, node.clone_id))
                    .or_default()
                    .push(NodeId::from_index(index));
            }
        }
        for members in members_by_stream.values().filter(|members| members.len() > 1) {
            let canonical = *members
                .iter()
                .min_by_key(|member| {
                    let node = self.node(**member);
                    (self.inside_dependency_store(**member), node.real_dev, node.file_id)
                })
                .expect("a clone group is not empty");
            let shares_with = u32::try_from(members.len() - 1).expect("clone group size fits in u32");
            for member in members {
                let node = &mut self.nodes[member.index()];
                node.shares_with = shares_with;
                if *member == canonical {
                    node.badges.insert(Badges::CLONE_CANONICAL);
                } else {
                    node.badges.insert(Badges::CLONE);
                    counted[member.index()] = false;
                }
            }
        }
        counted
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

    fn aggregate(&mut self, counted: &[bool]) {
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let is_directory = node.kind == NodeKind::Directory;
            node.allocated = if counted[index] { node.alloc_size } else { 0 };
            node.files = u32::from(!is_directory && node.kind != NodeKind::Unknown);
            node.newest_modified = if is_directory {
                Timestamp::UNKNOWN
            } else {
                node.modified
            };
            node.newest_opened = node.last_opened;
        }
        for index in (1..self.nodes.len()).rev() {
            let child = self.nodes[index].clone();
            let parent = &mut self.nodes[child.parent.index()];
            parent.allocated += child.allocated;
            parent.files += child.files;
            parent.newest_modified = parent.newest_modified.max(child.newest_modified);
            parent.newest_opened = parent.newest_opened.max(child.newest_opened);
        }
    }
}

fn may_share_blocks(node: &IndexNode) -> bool {
    node.ext_flags & u64::from(EF_MAY_SHARE_BLOCKS | EF_SHARES_ALL_BLOCKS) != 0
        || node.clone_refcount > 1
        || node.flags & UF_COMPRESSED != 0
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
    nodes.push(record(source, parent, interner));
    append_children(nodes, interner, id, source);
    id
}

fn append_children(nodes: &mut Vec<IndexNode>, interner: &mut NameInterner, id: NodeId, source: &Node) {
    let mut pending = vec![(id, source)];
    while let Some((parent, parent_source)) = pending.pop() {
        let mut children: Vec<&Node> = parent_source.children.iter().collect();
        children.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
        let first = NodeId::from_index(nodes.len());
        for child in &children {
            nodes.push(record(child, parent, interner));
        }
        let parent_node = &mut nodes[parent.index()];
        parent_node.first_child = first;
        parent_node.child_count = u32::try_from(children.len()).expect("child count fits in u32");
        for (offset, child) in children.into_iter().enumerate() {
            pending.push((NodeId::from_index(first.index() + offset), child));
        }
    }
}

fn record(source: &Node, parent: NodeId, interner: &mut NameInterner) -> IndexNode {
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
        name: interner.intern(&source.name),
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
