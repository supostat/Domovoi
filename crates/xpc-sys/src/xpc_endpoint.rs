use crate::sys::xpc_endpoint_create;
use crate::xpc_connection::XpcConnection;
use crate::xpc_object::XpcObject;

#[derive(Clone, Debug)]
pub struct XpcEndpoint {
    object: XpcObject,
}

impl XpcEndpoint {
    pub(crate) fn of(listener: &XpcConnection) -> Self {
        let raw = unsafe { xpc_endpoint_create(listener.as_object().as_raw().cast()) };
        Self {
            object: unsafe { XpcObject::from_owned(raw.cast()) },
        }
    }

    pub fn as_object(&self) -> &XpcObject {
        &self.object
    }

    pub fn connect(&self) -> XpcConnection {
        XpcConnection::from_endpoint(self)
    }
}
