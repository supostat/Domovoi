use std::io;

use domovoi_index::{Index, NodeId, Reclaimable};
use domovoi_rules::{DeleteRefusal, DeleteSubject, Denylist, Tray};

use crate::duplicate_copy::DuplicateCopy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub size: u64,
    pub keep: NodeId,
    pub copies: Vec<DuplicateCopy>,
    pub reclaimable: Reclaimable,
}

impl DuplicateGroup {
    pub fn of_members(index: &Index, size: u64, members: &[NodeId]) -> io::Result<Option<Self>> {
        let placed: Vec<(NodeId, Option<NodeId>)> = members
            .iter()
            .map(|member| (*member, index.enclosing_package(*member)))
            .collect();
        let (keep, _) = *placed
            .iter()
            .min_by_key(|(member, package)| {
                let node = index.node(*member);
                (
                    package.is_none(),
                    index.inside_trash(*member),
                    index.inside_dependency_store(*member),
                    node.real_dev,
                    node.file_id,
                    *member,
                )
            })
            .expect("a duplicate group has members");
        let copies: Vec<DuplicateCopy> = placed
            .iter()
            .filter(|(member, _)| *member != keep)
            .map(|(member, package)| DuplicateCopy {
                node: *member,
                shares_with_kept: shares_blocks_with_kept(index, keep, *member),
                package: *package,
            })
            .collect();
        if copies.iter().all(|copy| copy.package.is_some()) {
            return Ok(None);
        }
        let mut reclaimable = Reclaimable::default();
        for copy in copies.iter().filter(|copy| copy.is_removable()) {
            reclaimable += index.reclaimable(copy.node)?;
        }
        Ok(Some(Self {
            size,
            keep,
            copies,
            reclaimable,
        }))
    }

    pub fn take_copies_for_delete(
        &self,
        index: &Index,
        selected: &[NodeId],
        denylist: &Denylist,
    ) -> io::Result<(Vec<DeleteSubject>, Vec<DeleteRefusal>)> {
        let mut tray = Tray::default();
        for node in selected {
            assert!(
                self.copies.iter().any(|copy| copy.node == *node),
                "only copies of the group are deleted"
            );
            tray.add_duplicate_copy(index, *node, self.keep);
        }
        let sizes = tray
            .items()
            .iter()
            .map(|item| index.reclaimable(item.node))
            .collect::<io::Result<Vec<Reclaimable>>>()?;
        Ok(tray.take_for_delete(index, denylist, &sizes, |_| false))
    }
}

fn shares_blocks_with_kept(index: &Index, keep: NodeId, copy: NodeId) -> bool {
    let copy = index.node(copy);
    let keep = index.node(keep);
    keep.real_dev == copy.real_dev
        && (keep.file_id == copy.file_id || (copy.clone_id != 0 && keep.clone_id == copy.clone_id))
}
