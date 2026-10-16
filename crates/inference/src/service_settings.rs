use std::time::Duration;

use crate::backend_kind::BackendKind;
use crate::device_profile::DeviceProfile;
use crate::fake_backend::FakeBackend;
use crate::inference_backend::InferenceBackend;
use crate::llama_backend::LlamaBackend;

const BACKEND_ARGUMENT: &str = "--backend";
const BACKEND_VARIABLE: &str = "DOMOVOI_INFERENCE_BACKEND";
const DEVICES_VARIABLE: &str = "DOMOVOI_INFERENCE_DEVICES";
const IDLE_UNLOAD_VARIABLE: &str = "DOMOVOI_INFERENCE_IDLE_UNLOAD_SECS";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceSettings {
    pub backend: BackendKind,
    pub devices: DeviceProfile,
    pub idle_unload: Option<Duration>,
}

impl ServiceSettings {
    pub fn from_process(
        arguments: impl IntoIterator<Item = String>,
        variable: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let arguments: Vec<String> = arguments.into_iter().collect();
        let backend_name = match arguments.as_slice() {
            [] => variable(BACKEND_VARIABLE).unwrap_or_else(|| "llama".to_owned()),
            [flag, name] if flag == BACKEND_ARGUMENT => name.clone(),
            other => {
                return Err(format!(
                    "unexpected arguments {other:?}; use {BACKEND_ARGUMENT} llama|fake"
                ));
            }
        };
        let backend =
            BackendKind::from_name(&backend_name).ok_or_else(|| format!("unknown backend {backend_name:?}"))?;
        let devices_name = variable(DEVICES_VARIABLE).unwrap_or_else(|| "gpu".to_owned());
        let devices = DeviceProfile::from_name(&devices_name)
            .ok_or_else(|| format!("{DEVICES_VARIABLE} = {devices_name:?} is neither cpu nor gpu"))?;
        let idle_unload = variable(IDLE_UNLOAD_VARIABLE)
            .map(|seconds| {
                seconds
                    .trim()
                    .parse::<u64>()
                    .map(Duration::from_secs)
                    .map_err(|_| format!("{IDLE_UNLOAD_VARIABLE} = {seconds:?} is not a number of seconds"))
            })
            .transpose()?;
        Ok(Self {
            backend,
            devices,
            idle_unload,
        })
    }

    pub fn create_backend(self) -> Box<dyn InferenceBackend> {
        match self.backend {
            BackendKind::Llama => Box::new(LlamaBackend::new(self.devices)),
            BackendKind::Fake => Box::new(FakeBackend::default()),
        }
    }
}
