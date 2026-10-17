use serde::Deserialize;

use crate::duplicates_model::DuplicatesModel;
use crate::recommendations_model::RecommendationsModel;
use crate::tray_model::TrayModel;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CareModels {
    pub recommendations: RecommendationsModel,
    pub duplicates: DuplicatesModel,
    pub tray: TrayModel,
}

impl Default for CareModels {
    fn default() -> Self {
        Self {
            recommendations: RecommendationsModel::default(),
            duplicates: DuplicatesModel::idle(),
            tray: TrayModel::default(),
        }
    }
}
