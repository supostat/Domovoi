use std::ffi::c_void;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process;
use std::ptr::{self, NonNull};
use std::sync::Once;

use domovoi_inference_proto::{ErrorCode, GenerateRequest, GenerationStats, ModelClass};
use domovoi_llama_sys::{
    GGML_BACKEND_DEVICE_TYPE_GPU, GGML_TYPE_Q8_0, LLAMA_FLASH_ATTN_TYPE_ENABLED, LLAMA_LOAD_MODE_MMAP,
    ggml_backend_dev_by_type, ggml_backend_dev_t, llama_backend_init, llama_context_default_params,
    llama_init_from_model, llama_model_default_params, llama_model_free, llama_model_load_from_file_ptr,
};

use crate::backend_error::BackendError;
use crate::cancel_flag::CancelFlag;
use crate::device_profile::DeviceProfile;
use crate::inference_backend::InferenceBackend;
use crate::loaded_model::LoadedModel;
use crate::memory_budget::MemoryBudget;

pub(crate) const BATCH_SIZE: u32 = 256;
static BACKEND_INITIALISATION: Once = Once::new();

#[derive(Debug)]
pub struct LlamaBackend {
    profile: DeviceProfile,
    budget: Option<MemoryBudget>,
    model: Option<LoadedModel>,
}

impl LlamaBackend {
    pub fn new(profile: DeviceProfile) -> Self {
        BACKEND_INITIALISATION.call_once(|| unsafe { llama_backend_init() });
        Self {
            profile,
            budget: (profile == DeviceProfile::Gpu)
                .then(MemoryBudget::of_default_device)
                .flatten(),
            model: None,
        }
    }

    pub fn load_with_progress(
        &mut self,
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        on_progress: &mut dyn FnMut(f32) -> bool,
    ) -> Result<(), BackendError> {
        self.unload();
        if let Some(budget) = &self.budget
            && !budget.fits(class)
        {
            return Err(BackendError::new(
                ErrorCode::OutOfMemory,
                format!("the {class:?} class does not fit the GPU working set"),
            ));
        }
        let mut devices = self.devices()?;
        let descriptor = weights.into_raw_fd();
        let file = unsafe { libc::fdopen(descriptor, c"rb".as_ptr()) };
        if file.is_null() {
            drop(unsafe { OwnedFd::from_raw_fd(descriptor) });
            return Err(BackendError::new(
                ErrorCode::LoadFailed,
                "the weights descriptor cannot be read",
            ));
        }
        let mut aborted = false;
        let mut progress = |fraction: f32| {
            let proceed = on_progress(fraction);
            aborted |= !proceed;
            proceed
        };
        let mut progress_object: &mut dyn FnMut(f32) -> bool = &mut progress;
        let mut params = unsafe { llama_model_default_params() };
        params.load_mode = LLAMA_LOAD_MODE_MMAP;
        params.devices = devices.as_mut_ptr();
        params.progress_callback = Some(report_progress);
        params.progress_callback_user_data = (&raw mut progress_object).cast();
        let model = unsafe { llama_model_load_from_file_ptr(file, params) };
        unsafe { libc::fclose(file) };
        let Some(model) = NonNull::new(model) else {
            return Err(if aborted {
                BackendError::new(ErrorCode::LoadCancelled, "loading was cancelled")
            } else {
                BackendError::new(ErrorCode::LoadFailed, "llama.cpp could not load the weights")
            });
        };
        let mut context_params = unsafe { llama_context_default_params() };
        context_params.n_ctx = context_length;
        context_params.n_batch = BATCH_SIZE;
        context_params.n_ubatch = BATCH_SIZE;
        context_params.type_k = GGML_TYPE_Q8_0;
        context_params.type_v = GGML_TYPE_Q8_0;
        context_params.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
        context_params.no_perf = true;
        let context = unsafe { llama_init_from_model(model.as_ptr(), context_params) };
        let Some(context) = NonNull::new(context) else {
            unsafe { llama_model_free(model.as_ptr()) };
            return Err(BackendError::new(
                ErrorCode::LoadFailed,
                "llama.cpp could not create a context",
            ));
        };
        self.model = Some(unsafe { LoadedModel::new(model, context) });
        Ok(())
    }

    fn devices(&self) -> Result<[ggml_backend_dev_t; 2], BackendError> {
        match self.profile {
            DeviceProfile::Cpu => Ok([ptr::null_mut(); 2]),
            DeviceProfile::Gpu => {
                let gpu = unsafe { ggml_backend_dev_by_type(GGML_BACKEND_DEVICE_TYPE_GPU) };
                if gpu.is_null() {
                    return Err(BackendError::new(ErrorCode::LoadFailed, "no Metal device is available"));
                }
                Ok([gpu, ptr::null_mut()])
            }
        }
    }
}

impl InferenceBackend for LlamaBackend {
    fn load(
        &mut self,
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
        cancel: &CancelFlag,
    ) -> Result<(), BackendError> {
        self.load_with_progress(weights, class, context_length, &mut |_| !cancel.is_raised())
    }

    fn unload(&mut self) {
        self.model = None;
    }

    fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    fn generate(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        self.model
            .as_mut()
            .ok_or_else(|| BackendError::new(ErrorCode::ModelNotLoaded, "no model is loaded"))?
            .generate(request, cancel, on_token)
    }
}

unsafe extern "C" fn report_progress(fraction: f32, data: *mut c_void) -> bool {
    let callback = unsafe { &mut *data.cast::<&mut dyn FnMut(f32) -> bool>() };
    catch_unwind(AssertUnwindSafe(|| callback(fraction))).unwrap_or_else(|_| process::abort())
}
