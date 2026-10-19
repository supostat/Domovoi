use crate::rule_set::Claim;

pub(crate) enum ClaimScope {
    Inside(Claim),
    Open {
        components: Vec<String>,
        effective: Option<usize>,
    },
}
