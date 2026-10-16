use crate::sys::{xpc_object_t, xpc_session_cancel, xpc_session_send_message, xpc_session_t};
use crate::xpc_dictionary::XpcDictionary;
use crate::xpc_object::XpcObject;
use crate::xpc_rich_error::XpcRichError;

#[derive(Clone, Debug)]
pub struct PeerSession {
    object: XpcObject,
}

impl PeerSession {
    pub(crate) unsafe fn from_owned(raw: xpc_object_t) -> Self {
        Self {
            object: unsafe { XpcObject::from_owned(raw) },
        }
    }

    pub fn send(&self, message: &XpcDictionary) -> Result<(), XpcRichError> {
        let error = unsafe { xpc_session_send_message(self.raw(), message.as_object().as_raw()) };
        if error.is_null() {
            Ok(())
        } else {
            Err(unsafe { XpcRichError::from_owned(error) })
        }
    }

    pub fn cancel(&self) {
        unsafe { xpc_session_cancel(self.raw()) };
    }

    fn raw(&self) -> xpc_session_t {
        self.object.as_raw().cast()
    }
}
