#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CodeSigning {
    pub(crate) team_id: Option<String>,
    pub(crate) app_groups: Vec<String>,
}
