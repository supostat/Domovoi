use std::collections::BTreeSet;

use crate::installed_bundle::InstalledBundle;
use crate::owner_evidence::OwnerEvidence;

const TEAM_ID_LENGTH: usize = 10;
const GROUP_PREFIXES: [&str; 2] = ["group.", "groups."];
const SYSTEM_PREFIX: &str = "com.apple.";

pub(crate) fn group_owner(group: &str, bundles: &[InstalledBundle]) -> Option<(String, OwnerEvidence)> {
    let (team, rest) = match team_prefixed(group) {
        Some((team, rest)) => (Some(team), rest),
        None => (None, group),
    };
    if belongs_to_system(rest) {
        return None;
    }
    let listing: BTreeSet<&str> = bundles
        .iter()
        .filter(|bundle| bundle.app_groups.iter().any(|listed| listed == group))
        .map(|bundle| bundle.bundle_id.as_str())
        .collect();
    if !listing.is_empty() {
        return only(&listing).map(|owner| (owner.to_owned(), OwnerEvidence::AppGroup));
    }
    let team = team?;
    let team_bundles: BTreeSet<&str> = bundles
        .iter()
        .filter(|bundle| bundle.team_id.as_deref() == Some(team))
        .map(|bundle| bundle.bundle_id.as_str())
        .collect();
    match team_bundles.len() {
        0 => Some((group.to_owned(), OwnerEvidence::TeamPrefix)),
        _ => only(&team_bundles).map(|owner| (owner.to_owned(), OwnerEvidence::TeamPrefix)),
    }
}

fn team_prefixed(group: &str) -> Option<(&str, &str)> {
    let (team, rest) = group.split_once('.')?;
    let is_team = team.len() == TEAM_ID_LENGTH
        && team
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit());
    (is_team && !rest.is_empty()).then_some((team, rest))
}

fn belongs_to_system(rest: &str) -> bool {
    let unprefixed = GROUP_PREFIXES
        .iter()
        .find_map(|prefix| rest.strip_prefix(prefix))
        .unwrap_or(rest);
    unprefixed.starts_with(SYSTEM_PREFIX)
}

fn only<'name>(owners: &BTreeSet<&'name str>) -> Option<&'name str> {
    let mut owners = owners.iter();
    let owner = owners.next()?;
    owners.next().is_none().then_some(*owner)
}

#[cfg(test)]
mod tests {
    use super::group_owner;
    use crate::installed_bundle::InstalledBundle;
    use crate::owner_evidence::OwnerEvidence;

    fn bundle(bundle_id: &str, team_id: Option<&str>, app_groups: &[&str]) -> InstalledBundle {
        InstalledBundle {
            path: format!("/Applications/{bundle_id}.app"),
            bundle_id: bundle_id.to_owned(),
            name: bundle_id.to_owned(),
            team_id: team_id.map(str::to_owned),
            app_groups: app_groups.iter().map(|group| (*group).to_owned()).collect(),
        }
    }

    #[test]
    fn a_group_of_the_only_application_of_its_team_belongs_to_that_application() {
        let bundles = [bundle("com.vendor.app", Some("ABCDE12345"), &[])];
        assert_eq!(
            group_owner("ABCDE12345.com.vendor.shared", &bundles),
            Some(("com.vendor.app".to_owned(), OwnerEvidence::TeamPrefix))
        );
    }

    #[test]
    fn a_group_shared_by_several_applications_of_one_team_has_no_owner() {
        let bundles = [
            bundle("com.microsoft.Word", Some("UBF8T346G9"), &[]),
            bundle("com.microsoft.Excel", Some("UBF8T346G9"), &[]),
        ];
        assert_eq!(group_owner("UBF8T346G9.Office", &bundles), None);
    }

    #[test]
    fn the_entitlement_of_one_application_names_the_owner_among_its_team() {
        let bundles = [
            bundle(
                "com.microsoft.Word",
                Some("UBF8T346G9"),
                &["UBF8T346G9.com.microsoft.Word"],
            ),
            bundle("com.microsoft.Excel", Some("UBF8T346G9"), &[]),
        ];
        assert_eq!(
            group_owner("UBF8T346G9.com.microsoft.Word", &bundles),
            Some(("com.microsoft.Word".to_owned(), OwnerEvidence::AppGroup))
        );
    }

    #[test]
    fn a_group_of_a_team_without_applications_is_its_own_removed_owner() {
        let bundles = [bundle("com.vendor.app", Some("ABCDE12345"), &[])];
        assert_eq!(
            group_owner("ZYXWV98765.com.gone.data", &bundles),
            Some(("ZYXWV98765.com.gone.data".to_owned(), OwnerEvidence::TeamPrefix))
        );
    }

    #[test]
    fn system_groups_and_unlisted_groups_without_a_team_have_no_owner() {
        let bundles = [bundle("com.vendor.app", Some("ABCDE12345"), &[])];
        assert_eq!(group_owner("243LU875E5.groups.com.apple.podcasts", &bundles), None);
        assert_eq!(group_owner("group.com.apple.notes", &bundles), None);
        assert_eq!(group_owner("group.com.vendor.unlisted", &bundles), None);
    }
}
