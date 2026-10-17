use std::ffi::{CStr, CString};

use domovoi_xpc_sys::XpcEndpoint;

const INFERENCE_SERVICE: &CStr = c"com.domovoi.inference";

#[derive(Clone, Debug)]
pub enum ServiceAddress {
    Bundled(CString),
    Endpoint(XpcEndpoint),
}

impl ServiceAddress {
    pub fn bundled() -> Self {
        Self::Bundled(INFERENCE_SERVICE.to_owned())
    }
}
