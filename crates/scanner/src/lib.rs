#![deny(unsafe_code)]

mod cancel_token;
mod canonical_paths;
#[allow(unsafe_code)]
mod ffi;
mod full_disk_access;
mod node;
mod node_state;
mod onboarding;
mod protected_folders;
mod scan;
mod scan_error;
mod scan_options;
mod scan_progress;
mod scan_report;
mod volume_events;

pub use cancel_token::CancelToken;
pub use full_disk_access::FullDiskAccess;
pub use node::Node;
pub use node_state::NodeState;
pub use onboarding::Onboarding;
pub use protected_folders::ProtectedFolders;
pub use scan::{list_directory, scan, scan_entry};
pub use scan_error::ScanError;
pub use scan_options::ScanOptions;
pub use scan_progress::ScanProgress;
pub use scan_report::ScanReport;
pub use volume_events::{Since, VolumeEvent, VolumeEventStream, current_event_id, history_store};
