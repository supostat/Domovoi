use crate::app::AppId;
use crate::badges::Badges;
use crate::category::Category;
use crate::last_opened_source::LastOpenedSource;
use crate::name_table::NameId;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::reach::Reach;
use crate::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct IndexNode {
    pub name: NameId,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub child_count: u32,
    pub kind: NodeKind,
    pub reach: Reach,
    pub category: Category,
    pub badges: Badges,
    pub last_opened_source: LastOpenedSource,
    pub flags: u32,
    pub real_dev: u32,
    pub link_count: u32,
    pub clone_refcount: u32,
    pub shares_with: u32,
    pub files: u32,
    pub flags_in_subtree: u32,
    pub app: AppId,
    pub file_id: u64,
    pub link_id: u64,
    pub clone_id: u64,
    pub ext_flags: u64,
    pub alloc_size: u64,
    pub total_size: u64,
    pub allocated: u64,
    pub modified: Timestamp,
    pub created: Timestamp,
    pub added: Timestamp,
    pub last_opened: Timestamp,
    pub newest_modified: Timestamp,
    pub newest_opened: Timestamp,
}
