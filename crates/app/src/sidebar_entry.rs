use crate::section::Section;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarEntry {
    pub title: String,
    pub counter: Option<String>,
    pub children: Vec<SidebarEntry>,
    pub section: Option<Section>,
    pub selected: bool,
}
