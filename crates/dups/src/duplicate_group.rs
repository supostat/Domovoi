use std::io;

use domovoi_index::{Index, NodeId, Reclaimable};
use domovoi_rules::{Denylist, TrashJournal, TrashOutcome, Tray};

use crate::duplicate_copy::DuplicateCopy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub size: u64,
    pub fully_hashed: bool,
    pub keep: NodeId,
    pub copies: Vec<DuplicateCopy>,
    pub reclaimable: Reclaimable,
}

impl DuplicateGroup {
    pub fn of_members(index: &Index, size: u64, fully_hashed: bool, members: &[NodeId]) -> io::Result<Self> {
        let keep = *members
            .iter()
            .min_by_key(|member| {
                let node = index.node(**member);
                (
                    index.inside_dependency_store(**member),
                    node.real_dev,
                    node.file_id,
                    **member,
                )
            })
            .expect("a duplicate group has members");
        let copies: Vec<DuplicateCopy> = members
            .iter()
            .filter(|member| **member != keep)
            .map(|member| DuplicateCopy {
                node: *member,
                shares_blocks: shares_blocks_with_another_member(index, members, *member),
            })
            .collect();
        let mut reclaimable = Reclaimable::default();
        for copy in copies.iter().filter(|copy| !copy.shares_blocks) {
            reclaimable += index.reclaimable(copy.node)?;
        }
        Ok(Self {
            size,
            fully_hashed,
            keep,
            copies,
            reclaimable,
        })
    }

    pub fn move_copies_to_trash(
        &self,
        index: &Index,
        selected: &[NodeId],
        denylist: &Denylist,
        journal: &mut TrashJournal,
    ) -> io::Result<Vec<TrashOutcome>> {
        let mut tray = Tray::default();
        for node in selected {
            assert!(
                self.copies.iter().any(|copy| copy.node == *node),
                "only copies of the group go to the Trash"
            );
            tray.add(index, *node, None);
        }
        tray.move_to_trash(index, denylist, journal)
    }
}

fn shares_blocks_with_another_member(index: &Index, members: &[NodeId], member: NodeId) -> bool {
    let node = index.node(member);
    members.iter().filter(|other| **other != member).any(|other| {
        let other = index.node(*other);
        other.real_dev == node.real_dev
            && (other.file_id == node.file_id || (node.clone_id != 0 && other.clone_id == node.clone_id))
    })
}
