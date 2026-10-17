use std::os::fd::OwnedFd;

use domovoi_inference_proto::ModelClass;

#[derive(Debug)]
pub(crate) struct LoadedWeights {
    pub(crate) weights: OwnedFd,
    pub(crate) class: ModelClass,
    pub(crate) context_length: u32,
}
