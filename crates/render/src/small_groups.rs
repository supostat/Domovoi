use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmallGroups {
    pub always: bool,
    opened: BTreeSet<usize>,
}

impl SmallGroups {
    pub fn always(always: bool) -> Self {
        Self {
            always,
            opened: BTreeSet::new(),
        }
    }

    pub fn open(&mut self, parent: usize) {
        self.opened.insert(parent);
    }

    pub fn is_open(&self, parent: usize) -> bool {
        self.always || self.opened.contains(&parent)
    }
}
