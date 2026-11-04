use domovoi_rules::DenyReason;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropRefusal {
    pub name: String,
    pub reason: DenyReason,
}
