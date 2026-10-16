use std::ffi::c_void;

use block2::RcBlock;

use crate::handler_boundary::run_handler;
use crate::sys::{
    xpc_connection_activate, xpc_connection_cancel, xpc_connection_create_from_endpoint, xpc_connection_get_pid,
    xpc_connection_send_message, xpc_connection_send_message_with_reply_sync, xpc_connection_set_event_handler,
    xpc_connection_t, xpc_object_t,
};
use crate::xpc_dictionary::XpcDictionary;
use crate::xpc_endpoint::XpcEndpoint;
use crate::xpc_event::XpcEvent;
use crate::xpc_object::XpcObject;

#[derive(Clone, Debug)]
pub struct XpcConnection {
    object: XpcObject,
}

impl XpcConnection {
    pub fn from_endpoint(endpoint: &XpcEndpoint) -> Self {
        let raw = unsafe { xpc_connection_create_from_endpoint(endpoint.as_object().as_raw().cast()) };
        Self {
            object: unsafe { XpcObject::from_owned(raw.cast()) },
        }
    }

    pub(crate) fn from_object(object: XpcObject) -> Self {
        Self { object }
    }

    pub(crate) unsafe fn from_owned(raw: xpc_connection_t) -> Self {
        Self {
            object: unsafe { XpcObject::from_owned(raw.cast()) },
        }
    }

    pub fn set_event_handler(&self, handler: impl Fn(XpcEvent) + Send + Sync + 'static) {
        let block = RcBlock::new(move |event: xpc_object_t| {
            run_handler(|| handler(unsafe { XpcEvent::from_borrowed(event) }));
        });
        unsafe { xpc_connection_set_event_handler(self.raw(), RcBlock::as_ptr(&block).cast::<c_void>()) };
    }

    pub fn activate(&self) {
        unsafe { xpc_connection_activate(self.raw()) };
    }

    pub fn send(&self, message: &XpcDictionary) {
        unsafe { xpc_connection_send_message(self.raw(), message.as_object().as_raw()) };
    }

    pub fn send_with_reply_sync(&self, message: &XpcDictionary) -> XpcEvent {
        unsafe {
            XpcEvent::from_owned(xpc_connection_send_message_with_reply_sync(
                self.raw(),
                message.as_object().as_raw(),
            ))
        }
    }

    pub fn pid(&self) -> libc::pid_t {
        unsafe { xpc_connection_get_pid(self.raw()) }
    }

    pub fn cancel(&self) {
        unsafe { xpc_connection_cancel(self.raw()) };
    }

    pub(crate) fn as_object(&self) -> &XpcObject {
        &self.object
    }

    fn raw(&self) -> xpc_connection_t {
        self.object.as_raw().cast()
    }
}
