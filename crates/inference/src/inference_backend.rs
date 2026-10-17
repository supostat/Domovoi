use std::os::fd::OwnedFd;

use domovoi_inference_proto::{GenerateRequest, GenerationStats, ModelClass};

use crate::backend_error::BackendError;
use crate::cancel_flag::CancelFlag;

pub trait InferenceBackend: Send {
    fn load(
        &mut self,
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        cancel: &CancelFlag,
    ) -> Result<(), BackendError>;

    fn unload(&mut self);

    fn is_loaded(&self) -> bool;

    fn generate(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError>;

    fn count_tokens(&self, text: &str) -> Result<u32, BackendError>;
}
