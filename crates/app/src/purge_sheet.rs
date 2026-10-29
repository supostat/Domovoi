use objc2::rc::Retained;
use objc2_app_kit::NSWindow;

use crate::purge_model::PurgeModel;
use crate::purge_run::PurgeRun;

#[derive(Debug)]
pub(crate) struct PurgeSheet {
    pub window: Retained<NSWindow>,
    pub model: PurgeModel,
    pub run: Option<PurgeRun>,
}
