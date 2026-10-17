use domovoi_scanner::FullDiskAccess;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessState {
    Missing,
    GrantedAfterLaunch,
    Granted,
}

impl AccessState {
    pub fn probe() -> Self {
        match FullDiskAccess::probe() {
            FullDiskAccess::Granted => Self::Granted,
            FullDiskAccess::Denied | FullDiskAccess::Unknown => Self::Missing,
        }
    }
}
