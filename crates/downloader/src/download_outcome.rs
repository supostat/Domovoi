use domovoi_index::ModelEvent;

use crate::install_error::InstallError;

#[derive(Debug)]
pub enum DownloadOutcome {
    Installed(ModelEvent),
    Paused,
    Cancelled,
    Failed(InstallError),
}
