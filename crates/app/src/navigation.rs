use crate::section::Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Navigation {
    pub current: Section,
    pub care_ready: bool,
}

impl Navigation {
    pub fn allows(self, section: Section) -> bool {
        match section {
            Section::Disk => true,
            Section::Recommendations | Section::Duplicates => self.care_ready,
            Section::Assistant | Section::Model => false,
        }
    }
}
