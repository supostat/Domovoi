use domovoi_dups::{CloneReplaceError, Duplicates};
use domovoi_index::{Index, NodeId};
use domovoi_theme::{format_count, format_size};
use serde::Deserialize;

use crate::clone_refusal_text::clone_refusal_text;
use crate::duplicates_state::DuplicatesState;
use crate::group_model::GroupModel;
use crate::member_model::MemberModel;
use crate::strings::{CANDIDATES, CHECKED, GROUPS, MINUTES_SHORT, SECONDS_SHORT, WILL_FREE};
use crate::tilde::tilde;

const SECONDS_PER_MINUTE: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DuplicatesModel {
    #[serde(flatten)]
    pub state: DuplicatesState,
    #[serde(default)]
    pub clone_mode: bool,
}

impl DuplicatesModel {
    pub fn idle() -> Self {
        Self {
            state: DuplicatesState::Idle,
            clone_mode: false,
        }
    }

    pub fn found(index: &Index, duplicates: &Duplicates, candidates: u64, seconds: u64) -> std::io::Result<Self> {
        let mut groups = Vec::with_capacity(duplicates.groups.len());
        for group in &duplicates.groups {
            let keep_path = index.path(group.keep);
            let mut copies = Vec::with_capacity(group.copies.len());
            for copy in &group.copies {
                let frees = if copy.shares_blocks {
                    0
                } else {
                    index.reclaimable(copy.node)?.bytes
                };
                copies.push(MemberModel {
                    path: tilde(&index.path(copy.node)),
                    size: group.size,
                    frees,
                    clone: copy.shares_blocks,
                    checked: !copy.shares_blocks,
                    refusal: None,
                    node: Some(copy.node),
                });
            }
            groups.push(GroupModel {
                name: keep_path
                    .file_name()
                    .map_or_else(|| tilde(&keep_path), |name| name.to_string_lossy().into_owned()),
                keep: MemberModel {
                    path: tilde(&keep_path),
                    size: group.size,
                    frees: 0,
                    clone: false,
                    checked: false,
                    refusal: None,
                    node: Some(group.keep),
                },
                copies,
            });
        }
        Ok(Self {
            state: DuplicatesState::Found {
                candidates,
                seconds,
                groups,
            },
            clone_mode: false,
        })
    }

    pub fn groups(&self) -> &[GroupModel] {
        match &self.state {
            DuplicatesState::Found { groups, .. } => groups,
            DuplicatesState::Idle | DuplicatesState::Searching { .. } => &[],
        }
    }

    pub fn groups_mut(&mut self) -> &mut [GroupModel] {
        match &mut self.state {
            DuplicatesState::Found { groups, .. } => groups,
            DuplicatesState::Idle | DuplicatesState::Searching { .. } => &mut [],
        }
    }

    pub fn will_free(&self) -> u64 {
        let clone_mode = self.clone_mode;
        self.groups()
            .iter()
            .flat_map(|group| &group.copies)
            .filter(|copy| copy.checked && copy.can_check(clone_mode))
            .map(|copy| copy.frees)
            .sum()
    }

    pub fn checked_copies(&self) -> Vec<(NodeId, NodeId)> {
        let clone_mode = self.clone_mode;
        self.groups()
            .iter()
            .flat_map(|group| {
                group
                    .copies
                    .iter()
                    .filter(move |copy| copy.checked && copy.can_check(clone_mode))
                    .filter_map(move |copy| Some((group.keep.node?, copy.node?)))
            })
            .collect()
    }

    pub fn title_line(&self) -> Option<String> {
        let groups = self.groups();
        matches!(self.state, DuplicatesState::Found { .. }).then(|| {
            format!(
                "{} · {WILL_FREE} {}",
                format_count(groups.len() as u64, &GROUPS),
                format_size(self.will_free())
            )
        })
    }

    pub fn checked_line(&self) -> Option<String> {
        match self.state {
            DuplicatesState::Found {
                candidates, seconds, ..
            } => Some(format!(
                "{CHECKED} {} · {} {MINUTES_SHORT} {} {SECONDS_SHORT}",
                format_count(candidates, &CANDIDATES),
                seconds / SECONDS_PER_MINUTE,
                seconds % SECONDS_PER_MINUTE
            )),
            DuplicatesState::Idle | DuplicatesState::Searching { .. } => None,
        }
    }

    pub fn assess_clones(&mut self, refusal: impl Fn(NodeId, NodeId) -> Result<(), CloneReplaceError>) {
        for group in self.groups_mut() {
            let Some(keep) = group.keep.node else {
                continue;
            };
            for copy in group.copies.iter_mut().filter(|copy| !copy.clone) {
                copy.refusal = copy
                    .node
                    .and_then(|node| refusal(keep, node).err())
                    .map(|error| clone_refusal_text(&error).to_owned());
            }
        }
    }

    pub fn clone_available(&self) -> bool {
        self.groups()
            .iter()
            .flat_map(|group| &group.copies)
            .any(|copy| copy.can_check(true))
    }
}
