use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use domovoi_apfs_sys::sys::UF_COMPRESSED;
use domovoi_scanner::Node;

use crate::badges::Badges;
use crate::delta_applied::DeltaApplied;
use crate::delta_request::DeltaRequest;
use crate::directory_listing::DirectoryListing;
use crate::enrichment::Enrichment;
use crate::file_types::FileTypes;
use crate::index::{Index, add_child_totals, append_children, record, totals};
use crate::index_node::IndexNode;
use crate::kept_child::KeptChild;
use crate::last_opened_source::LastOpenedSource;
use crate::listed_entry::ListedEntry;
use crate::name_table::NameInterner;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::path_rules::RuleProgress;
use crate::spliced_directory::SplicedDirectory;
use crate::stale_delta::StaleDelta;
use crate::timestamp::Timestamp;

struct Splice {
    directory: NodeId,
    added: Vec<NodeId>,
    removed: Vec<NodeId>,
    kept: Vec<KeptChild>,
    own_totals_changed: Vec<NodeId>,
    appeared: Vec<OsString>,
    vanished: Vec<OsString>,
}

enum Placement<'listing> {
    Kept { source: &'listing Node, previous: NodeId },
    Read(&'listing Node),
}

impl Placement<'_> {
    fn name(&self) -> &OsStr {
        match self {
            Self::Kept { source, .. } | Self::Read(source) => &source.name,
        }
    }
}

impl Index {
    pub fn apply_delta(
        &mut self,
        request: &DeltaRequest,
        mut listings: Vec<DirectoryListing>,
        enrichment: &Enrichment,
    ) -> Result<DeltaApplied, StaleDelta> {
        if request.generation != self.generation {
            return Err(StaleDelta);
        }
        listings.sort_by_cached_key(|listing| Reverse(self.depth_of(listing.directory)));
        let spotlight: HashMap<&Path, SystemTime> = enrichment
            .spotlight_last_used
            .iter()
            .map(|(path, date)| (path.as_path(), *date))
            .collect();
        let mut spliced = Vec::with_capacity(listings.len());
        let mut regrouped = HashSet::new();
        for listing in &listings {
            let splice = self.splice(listing);
            self.categorize_splice(&splice, enrichment);
            self.mark_models(enrichment);
            self.apply_spotlight_dates_to(&splice.added, &spotlight);
            let counted = self.regroup(&splice.removed, &splice.added);
            self.reaggregate(&splice, &counted);
            self.hidden_folders =
                self.hidden_folders + self.count_hidden(&splice.added) - self.count_hidden(&splice.removed);
            self.garbage += splice.removed.len() + splice.kept.len();
            let added: HashSet<NodeId> = splice.added.iter().copied().collect();
            regrouped.extend(counted.into_keys().filter(|id| !added.contains(id)));
            spliced.push(SplicedDirectory {
                directory: splice.directory,
                kept: splice.kept,
                appeared: splice.appeared,
                vanished: splice.vanished,
            });
        }
        if !spliced.is_empty() {
            self.advance_generation();
        }
        let directories = self.surviving(spliced);
        if self.garbage >= self.len() {
            let paths: Vec<PathBuf> = directories.iter().map(|spliced| self.path(spliced.directory)).collect();
            self.compact_garbage();
            return Ok(DeltaApplied::Compacted {
                directories: paths
                    .iter()
                    .map(|path| self.find(path).expect("a live directory survives compaction"))
                    .collect(),
            });
        }
        Ok(DeltaApplied::Spliced {
            directories,
            regrouped: regrouped.into_iter().filter(|id| self.is_live(*id)).collect(),
        })
    }

    fn surviving(&self, spliced: Vec<SplicedDirectory>) -> Vec<SplicedDirectory> {
        let relocated: HashMap<NodeId, NodeId> = spliced
            .iter()
            .flat_map(|directory| directory.kept.iter().map(|kept| (kept.previous, kept.current)))
            .collect();
        spliced
            .into_iter()
            .map(|directory| SplicedDirectory {
                directory: relocated
                    .get(&directory.directory)
                    .copied()
                    .unwrap_or(directory.directory),
                ..directory
            })
            .filter(|directory| self.is_live(directory.directory))
            .collect()
    }

    fn splice(&mut self, listing: &DirectoryListing) -> Splice {
        let directory = listing.directory;
        let mut entries: Vec<&ListedEntry> = listing.entries.iter().collect();
        entries.sort_by(|left, right| left.node().name.as_bytes().cmp(right.node().name.as_bytes()));
        let placements: Vec<Placement<'_>> = entries
            .into_iter()
            .map(|entry| match entry {
                ListedEntry::Kept(source) => Placement::Kept {
                    source,
                    previous: self
                        .child_named(directory, &source.name)
                        .expect("a kept folder is still a child of its directory"),
                },
                ListedEntry::Read(source) => Placement::Read(source),
            })
            .collect();
        let kept: HashSet<NodeId> = placements
            .iter()
            .filter_map(|placement| match placement {
                Placement::Kept { previous, .. } => Some(*previous),
                Placement::Read(_) => None,
            })
            .collect();
        let removed: Vec<NodeId> = self
            .children(directory)
            .filter(|child| !kept.contains(child))
            .flat_map(|child| self.subtree(child))
            .collect();
        let (appeared, vanished) = self.name_changes(directory, &placements);
        let mut interner = NameInterner::appending(mem::take(&mut self.names));
        let first = self.nodes.len();
        let mut kept = Vec::new();
        let mut own_totals_changed = Vec::new();
        let mut read = Vec::new();
        for placement in &placements {
            let id = NodeId::from_index(self.nodes.len());
            match placement {
                Placement::Kept { source, previous } => {
                    let record_before = &self.nodes[previous.index()];
                    let fresh = record(source, directory, record_before.name);
                    if fresh.alloc_size != record_before.alloc_size || fresh.flags != record_before.flags {
                        own_totals_changed.push(id);
                    }
                    let kept_node = kept_record(record_before, &fresh);
                    self.nodes.push(kept_node);
                    kept.push(KeptChild {
                        previous: *previous,
                        current: id,
                    });
                }
                Placement::Read(source) => {
                    let name = interner.intern(&source.name);
                    self.nodes.push(record(source, directory, name));
                    read.push((id, *source));
                }
            }
        }
        let node = &mut self.nodes[directory.index()];
        node.first_child = NodeId::from_index(first);
        node.child_count = u32::try_from(placements.len()).expect("child count fits in u32");
        for (id, source) in read {
            append_children(&mut self.nodes, &mut interner, id, source);
        }
        self.names = interner.finish();
        for kept_child in &kept {
            let children: Vec<NodeId> = self.children(kept_child.current).collect();
            for child in children {
                self.nodes[child.index()].parent = kept_child.current;
            }
        }
        let relocated: HashSet<NodeId> = kept.iter().map(|kept_child| kept_child.current).collect();
        let added = (first..self.nodes.len())
            .map(NodeId::from_index)
            .filter(|id| !relocated.contains(id))
            .collect();
        Splice {
            directory,
            added,
            removed,
            kept,
            own_totals_changed,
            appeared,
            vanished,
        }
    }

    fn name_changes(&self, directory: NodeId, placements: &[Placement<'_>]) -> (Vec<OsString>, Vec<OsString>) {
        let mut before = self.children(directory).map(|child| self.name(child)).peekable();
        let mut after = placements.iter().map(Placement::name).peekable();
        let (mut appeared, mut vanished) = (Vec::new(), Vec::new());
        loop {
            match (before.peek(), after.peek()) {
                (None, None) => break,
                (Some(old), Some(new)) => match old.as_bytes().cmp(new.as_bytes()) {
                    Ordering::Equal => {
                        before.next();
                        after.next();
                    }
                    Ordering::Less => vanished.extend(before.next().map(OsStr::to_owned)),
                    Ordering::Greater => appeared.extend(after.next().map(OsStr::to_owned)),
                },
                (Some(_), None) => vanished.extend(before.next().map(OsStr::to_owned)),
                (None, Some(_)) => appeared.extend(after.next().map(OsStr::to_owned)),
            }
        }
        (appeared, vanished)
    }

    fn categorize_splice(&mut self, splice: &Splice, enrichment: &Enrichment) {
        let mut file_types = FileTypes::default();
        let directory = splice.directory;
        let (directory_progress, directory_depth) = match self.parent(directory) {
            Some(parent) => {
                let (parent_progress, parent_depth) = self.rule_progress_of(parent);
                let before = self.node(directory).category;
                let progress = self.categorize(directory, parent_progress, parent_depth, &mut file_types, enrichment);
                if self.node(directory).category != before {
                    let subtree = self.subtree(directory);
                    self.categorize_descendants(
                        &subtree[1..],
                        (directory, progress, parent_depth + 1),
                        &mut file_types,
                        enrichment,
                    );
                    return;
                }
                (progress, parent_depth + 1)
            }
            None => (self.root_rules().0, self.root.root_components().len()),
        };
        self.categorize_descendants(
            &splice.added,
            (directory, directory_progress, directory_depth),
            &mut file_types,
            enrichment,
        );
    }

    fn categorize_descendants(
        &mut self,
        parents_first: &[NodeId],
        (top, top_progress, top_depth): (NodeId, RuleProgress, usize),
        file_types: &mut FileTypes,
        enrichment: &Enrichment,
    ) {
        let mut progress: HashMap<NodeId, (RuleProgress, usize)> = HashMap::new();
        progress.insert(top, (top_progress, top_depth));
        for id in parents_first {
            let parent = self.node(*id).parent;
            let (parent_progress, parent_depth) = progress[&parent];
            let own = self.categorize(*id, parent_progress, parent_depth, file_types, enrichment);
            progress.insert(*id, (own, parent_depth + 1));
        }
    }

    fn apply_spotlight_dates_to(&mut self, added: &[NodeId], spotlight: &HashMap<&Path, SystemTime>) {
        if spotlight.is_empty() {
            return;
        }
        for id in added {
            if self.node(*id).kind != NodeKind::Directory {
                continue;
            }
            if let Some(date) = spotlight.get(self.path(*id).as_path()) {
                let node = &mut self.nodes[id.index()];
                node.last_opened = Timestamp::from_system_time(*date);
                node.last_opened_source = LastOpenedSource::SpotlightIndex;
            }
        }
    }

    fn reaggregate(&mut self, splice: &Splice, counted: &HashMap<NodeId, bool>) {
        let added: HashSet<NodeId> = splice.added.iter().copied().collect();
        for id in &splice.added {
            let own_counted = counted
                .get(id)
                .copied()
                .unwrap_or(self.node(*id).kind != NodeKind::Unknown);
            self.reset_own_totals(*id, own_counted);
        }
        for id in splice.added.iter().rev() {
            let child = self.nodes[id.index()].clone();
            if added.contains(&child.parent) {
                add_child_totals(&mut self.nodes[child.parent.index()], &child);
            }
        }
        let mut dirty: HashSet<NodeId> = splice.own_totals_changed.iter().copied().collect();
        dirty.insert(splice.directory);
        for (id, is_counted) in counted {
            if added.contains(id) {
                continue;
            }
            let node = &mut self.nodes[id.index()];
            let own = if *is_counted { node.alloc_size } else { 0 };
            if node.allocated != own {
                node.allocated = own;
                dirty.insert(node.parent);
            }
        }
        self.refresh_totals(dirty);
    }

    fn refresh_totals(&mut self, dirty: HashSet<NodeId>) {
        let mut queue: BinaryHeap<(usize, NodeId)> = dirty.iter().map(|id| (self.depth_of(*id), *id)).collect();
        let mut queued = dirty;
        while let Some((depth, id)) = queue.pop() {
            if self.recompute_totals(id)
                && let Some(parent) = self.parent(id)
                && queued.insert(parent)
            {
                queue.push((depth - 1, parent));
            }
        }
    }

    fn recompute_totals(&mut self, id: NodeId) -> bool {
        let before = totals(self.node(id));
        let counted = self.node(id).kind != NodeKind::Unknown;
        self.reset_own_totals(id, counted);
        let children: Vec<NodeId> = self.children(id).collect();
        for child in children {
            let child = self.nodes[child.index()].clone();
            add_child_totals(&mut self.nodes[id.index()], &child);
        }
        totals(self.node(id)) != before
    }
}

fn kept_record(previous: &IndexNode, fresh: &IndexNode) -> IndexNode {
    let mut badges = previous.badges;
    if fresh.flags & UF_COMPRESSED != 0 {
        badges.insert(Badges::COMPRESSED);
    } else {
        badges.remove(Badges::COMPRESSED);
    }
    IndexNode {
        name: previous.name,
        first_child: previous.first_child,
        child_count: previous.child_count,
        category: previous.category,
        app: previous.app,
        badges,
        shares_with: previous.shares_with,
        last_opened: previous.last_opened,
        last_opened_source: previous.last_opened_source,
        allocated: previous.allocated,
        files: previous.files,
        newest_modified: previous.newest_modified,
        newest_opened: previous.newest_opened,
        flags_in_subtree: previous.flags_in_subtree,
        ..*fresh
    }
}
