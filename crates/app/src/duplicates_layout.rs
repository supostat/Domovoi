use crate::group_model::GroupModel;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DuplicatesLayout {
    pub toggles: Vec<(usize, usize)>,
}

impl DuplicatesLayout {
    pub fn of(groups: &[GroupModel]) -> Self {
        Self {
            toggles: groups
                .iter()
                .enumerate()
                .flat_map(|(group, model)| (0..model.copies.len()).map(move |copy| (group, copy)))
                .collect(),
        }
    }

    pub fn tag(&self, group: usize, copy: usize) -> usize {
        self.toggles
            .iter()
            .position(|toggle| *toggle == (group, copy))
            .expect("every copy has a toggle")
    }
}
