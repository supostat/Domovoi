use std::cmp::Reverse;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::TrySendError;
use std::time::Instant;

use domovoi_apfs_sys::set_vfs_policies;
use domovoi_apfs_sys::sys::SF_DATALESS;
use domovoi_index::{Index, NodeId, NodeKind, Reach};
use rayon::prelude::*;

use crate::bucket::Bucket;
use crate::candidate::Candidate;
use crate::content_digest::{digest, edges_cover_whole_file};
use crate::digest_kind::DigestKind;
use crate::duplicate_group::DuplicateGroup;
use crate::duplicate_options::DuplicateOptions;
use crate::duplicate_progress::DuplicateProgress;
use crate::duplicate_stage::DuplicateStage;
use crate::duplicates::Duplicates;
use crate::hashing_pool::HashingPool;
use crate::search_error::SearchError;
use crate::skip_reason::SkipReason;
use crate::skipped_file::SkippedFile;

pub fn find_duplicates(
    index: &Index,
    pool: &HashingPool,
    options: &DuplicateOptions,
) -> Result<Duplicates, SearchError> {
    set_vfs_policies().map_err(SearchError::Io)?;
    let mut skipped = Vec::new();
    let candidates = candidates(index, options, &mut skipped);
    let count = candidates.len();
    let mut search = Search {
        pool,
        options,
        candidates,
        edges: vec![None; count],
        contents: vec![None; count],
        failures: vec![None; count],
    };
    let regular: Vec<usize> = (0..count)
        .filter(|position| !search.candidates[*position].in_cloud_folder)
        .collect();
    let mut buckets = search.refine(&regular, DuplicateStage::Edges, DuplicateStage::Contents, None)?;
    if regular.len() < count {
        let deadline = Instant::now().checked_add(options.cloud_time_limit);
        let everything: Vec<usize> = (0..count).collect();
        buckets = search.refine(
            &everything,
            DuplicateStage::CloudFolders,
            DuplicateStage::CloudFolders,
            deadline,
        )?;
    }
    for (candidate, failure) in search.candidates.iter().zip(&search.failures) {
        if let Some(reason) = failure {
            skipped.extend(candidate.entries.iter().map(|node| SkippedFile {
                node: *node,
                reason: *reason,
            }));
        }
    }
    let mut groups = Vec::with_capacity(buckets.len());
    for bucket in &buckets {
        let members: Vec<NodeId> = bucket
            .members
            .iter()
            .flat_map(|position| search.candidates[*position].entries.iter().copied())
            .collect();
        groups.push(
            DuplicateGroup::of_members(index, bucket.size, bucket.fully_hashed, &members).map_err(SearchError::Io)?,
        );
    }
    groups.sort_by_key(|group| (Reverse(group.reclaimable.bytes), Reverse(group.size), group.keep));
    Ok(Duplicates { groups, skipped })
}

struct Search<'search> {
    pool: &'search HashingPool,
    options: &'search DuplicateOptions,
    candidates: Vec<Candidate>,
    edges: Vec<Option<blake3::Hash>>,
    contents: Vec<Option<blake3::Hash>>,
    failures: Vec<Option<SkipReason>>,
}

impl Search<'_> {
    fn refine(
        &mut self,
        members: &[usize],
        edge_stage: DuplicateStage,
        content_stage: DuplicateStage,
        deadline: Option<Instant>,
    ) -> Result<Vec<Bucket>, SearchError> {
        let by_size = shared_keys(
            members
                .iter()
                .map(|position| (self.candidates[*position].size, *position)),
        );
        let wanting_edges: Vec<usize> = by_size
            .values()
            .flatten()
            .copied()
            .filter(|position| self.edges[*position].is_none() && self.failures[*position].is_none())
            .collect();
        self.hash(&wanting_edges, DigestKind::Edges, edge_stage, deadline)?;
        let by_edges = shared_keys(by_size.values().flatten().filter_map(|position| {
            self.edges[*position].map(|edge| ((self.candidates[*position].size, edge), *position))
        }));
        let wanting_contents: Vec<usize> = by_edges
            .iter()
            .filter(|((size, _), _)| self.needs_contents(*size))
            .flat_map(|(_, positions)| positions.iter().copied())
            .filter(|position| self.contents[*position].is_none() && self.failures[*position].is_none())
            .collect();
        self.hash(&wanting_contents, DigestKind::Contents, content_stage, deadline)?;
        let mut buckets = Vec::new();
        for ((size, _), positions) in by_edges {
            if !self.needs_contents(size) {
                buckets.push(Bucket {
                    size,
                    fully_hashed: edges_cover_whole_file(size),
                    members: positions,
                });
                continue;
            }
            let by_contents = shared_keys(
                positions
                    .iter()
                    .filter_map(|position| self.contents[*position].map(|content| (content, *position))),
            );
            buckets.extend(by_contents.into_values().map(|members| Bucket {
                size,
                fully_hashed: true,
                members,
            }));
        }
        Ok(buckets)
    }

    fn needs_contents(&self, size: u64) -> bool {
        !edges_cover_whole_file(size) && size <= self.options.full_hash_limit
    }

    fn hash(
        &mut self,
        wanted: &[usize],
        kind: DigestKind,
        stage: DuplicateStage,
        deadline: Option<Instant>,
    ) -> Result<(), SearchError> {
        if wanted.is_empty() {
            return Ok(());
        }
        let options = self.options;
        let candidates = &self.candidates;
        let total = wanted.len() as u64;
        let done = AtomicU64::new(0);
        let outcomes: Vec<(usize, Option<Result<blake3::Hash, SkipReason>>)> = self.pool.install(|| {
            wanted
                .par_iter()
                .map(|position| {
                    if options.cancel.is_cancelled() {
                        return (*position, None);
                    }
                    let candidate = &candidates[*position];
                    let outcome = if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        Err(SkipReason::TimeLimit)
                    } else {
                        digest(&candidate.physical, candidate.size, kind)
                    };
                    report(
                        options,
                        DuplicateProgress {
                            stage,
                            files_done: done.fetch_add(1, Ordering::Relaxed) + 1,
                            files_total: total,
                        },
                    );
                    (*position, Some(outcome))
                })
                .collect()
        });
        if options.cancel.is_cancelled() {
            return Err(SearchError::Cancelled);
        }
        for (position, outcome) in outcomes {
            match outcome.expect("every file is hashed when the search is not cancelled") {
                Ok(hash) => match kind {
                    DigestKind::Edges => self.edges[position] = Some(hash),
                    DigestKind::Contents => self.contents[position] = Some(hash),
                },
                Err(reason) => self.failures[position] = Some(reason),
            }
        }
        Ok(())
    }
}

fn candidates(index: &Index, options: &DuplicateOptions, skipped: &mut Vec<SkippedFile>) -> Vec<Candidate> {
    let mut inodes: HashMap<(u32, u64), Vec<NodeId>> = HashMap::new();
    let mut pending = vec![index.root()];
    while let Some(id) = pending.pop() {
        pending.extend(index.children(id));
        let node = index.node(id);
        if node.kind != NodeKind::File || node.total_size < options.minimum_size {
            continue;
        }
        if node.reach == Reach::InCloud || node.flags & SF_DATALESS != 0 {
            skipped.push(SkippedFile {
                node: id,
                reason: SkipReason::InCloud,
            });
        } else {
            inodes.entry((node.real_dev, node.file_id)).or_default().push(id);
        }
    }
    let mut inodes_by_size: HashMap<u64, usize> = HashMap::new();
    for entries in inodes.values() {
        *inodes_by_size.entry(index.node(entries[0]).total_size).or_default() += 1;
    }
    let mut candidates: Vec<Candidate> = inodes
        .into_values()
        .filter(|entries| inodes_by_size[&index.node(entries[0]).total_size] > 1)
        .map(|mut entries| {
            entries.sort_unstable();
            let first = entries[0];
            Candidate {
                size: index.node(first).total_size,
                physical: index.physical_path(first),
                in_cloud_folder: options.cloud_folders.contains(&index.path(first)),
                entries,
            }
        })
        .collect();
    candidates.sort_unstable_by_key(|candidate| candidate.entries[0]);
    candidates
}

fn shared_keys<K: Eq + Hash>(keyed: impl Iterator<Item = (K, usize)>) -> HashMap<K, Vec<usize>> {
    let mut groups: HashMap<K, Vec<usize>> = HashMap::new();
    for (key, position) in keyed {
        groups.entry(key).or_default().push(position);
    }
    groups.retain(|_, positions| positions.len() > 1);
    for positions in groups.values_mut() {
        positions.sort_unstable();
    }
    groups
}

fn report(options: &DuplicateOptions, progress: DuplicateProgress) {
    if let Some(sender) = &options.progress {
        match sender.try_send(progress) {
            Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
        }
    }
}
