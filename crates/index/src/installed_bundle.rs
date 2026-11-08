use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use objc2_foundation::NSString;

use crate::ffi::{code_signing, property_list_dictionary};
use crate::property_list::text;

const BUNDLE_SUFFIX: &[u8] = b".app";
pub(crate) const INFO_PLIST: &str = "Contents/Info.plist";
const IDENTIFIER_KEY: &str = "CFBundleIdentifier";
const DISPLAY_NAME_KEY: &str = "CFBundleDisplayName";
const NAME_KEY: &str = "CFBundleName";

#[derive(Clone, Debug, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct InstalledBundle {
    pub path: String,
    pub bundle_id: String,
    pub name: String,
    pub team_id: Option<String>,
    pub app_groups: Vec<String>,
}

impl InstalledBundle {
    pub(crate) fn read(physical: &Path, canonical: &Path) -> Option<Self> {
        let info = property_list_dictionary(&physical.join(INFO_PLIST))?;
        let bundle_id = text(&info, &NSString::from_str(IDENTIFIER_KEY)).filter(|identifier| !identifier.is_empty())?;
        let name = text(&info, &NSString::from_str(DISPLAY_NAME_KEY))
            .or_else(|| text(&info, &NSString::from_str(NAME_KEY)))
            .unwrap_or_else(|| file_stem(canonical));
        let signing = code_signing(physical).unwrap_or_default();
        Some(Self {
            path: canonical.to_string_lossy().into_owned(),
            bundle_id,
            name,
            team_id: signing.team_id,
            app_groups: signing.app_groups,
        })
    }

    pub(crate) fn is_bundle_name(name: &OsStr) -> bool {
        name.as_bytes().ends_with(BUNDLE_SUFFIX)
    }

    pub(crate) fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.path)
    }
}

fn file_stem(bundle: &Path) -> String {
    bundle
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}
