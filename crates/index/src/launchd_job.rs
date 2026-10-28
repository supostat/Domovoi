use std::path::Path;

use objc2_foundation::NSString;

use crate::ffi::property_list_dictionary;
use crate::installed_bundle::InstalledBundle;
use crate::owner_claim::bundle_id_named;
use crate::owner_evidence::OwnerEvidence;
use crate::property_list::{text, texts};

const ASSOCIATED_BUNDLE_IDENTIFIERS_KEY: &str = "AssociatedBundleIdentifiers";
const PROGRAM_KEY: &str = "Program";
const PROGRAM_ARGUMENTS_KEY: &str = "ProgramArguments";
const LABEL_KEY: &str = "Label";

pub(crate) fn launchd_owner(plist: &Path, bundles: &[InstalledBundle]) -> Option<(String, OwnerEvidence)> {
    let job = property_list_dictionary(plist)?;
    if let Some(associated) = texts(&job, &NSString::from_str(ASSOCIATED_BUNDLE_IDENTIFIERS_KEY))
        .into_iter()
        .next()
    {
        return Some((associated, OwnerEvidence::LaunchdAssociation));
    }
    let program = text(&job, &NSString::from_str(PROGRAM_KEY)).or_else(|| {
        texts(&job, &NSString::from_str(PROGRAM_ARGUMENTS_KEY))
            .into_iter()
            .next()
    });
    let launching = program.and_then(|program| bundles.iter().find(|bundle| bundle.contains(Path::new(&program))));
    if let Some(bundle) = launching {
        return Some((bundle.bundle_id.clone(), OwnerEvidence::LaunchdAssociation));
    }
    text(&job, &NSString::from_str(LABEL_KEY)).and_then(|label| bundle_id_named(&label))
}
