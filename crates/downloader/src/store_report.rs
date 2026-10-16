use crate::installed_model::InstalledModel;
use crate::orphan::Orphan;
use crate::partial_download::PartialDownload;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoreReport {
    pub installed: Vec<InstalledModel>,
    pub partials: Vec<PartialDownload>,
    pub orphans: Vec<Orphan>,
}
