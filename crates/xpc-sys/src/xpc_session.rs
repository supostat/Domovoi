use std::ffi::{CStr, c_void};
use std::ptr;

use block2::RcBlock;

use crate::handler_boundary::run_handler;
use crate::sys::{
    XPC_SESSION_CREATE_INACTIVE, xpc_object_t, xpc_session_activate, xpc_session_cancel,
    xpc_session_create_xpc_service, xpc_session_send_message, xpc_session_send_message_with_reply_sync,
    xpc_session_set_cancel_handler, xpc_session_set_incoming_message_handler, xpc_session_t,
};
use crate::xpc_dictionary::XpcDictionary;
use crate::xpc_object::XpcObject;
use crate::xpc_rich_error::XpcRichError;

#[derive(Debug)]
pub struct XpcSession {
    object: XpcObject,
}

impl XpcSession {
    pub fn connect_service(
        name: &CStr,
        on_message: impl Fn(XpcDictionary) + Send + Sync + 'static,
        on_cancel: impl Fn(XpcRichError) + Send + Sync + 'static,
    ) -> Result<Self, XpcRichError> {
        let mut error = ptr::null_mut();
        let raw = unsafe {
            xpc_session_create_xpc_service(
                name.as_ptr(),
                ptr::null_mut(),
                XPC_SESSION_CREATE_INACTIVE,
                &raw mut error,
            )
        };
        if raw.is_null() {
            return Err(unsafe { XpcRichError::from_owned(error) });
        }
        let session = Self {
            object: unsafe { XpcObject::from_owned(raw.cast()) },
        };
        let message_handler = RcBlock::new(move |message: xpc_object_t| {
            run_handler(|| {
                let object = unsafe { XpcObject::from_borrowed(message) };
                if let Some(message) = XpcDictionary::from_object(object) {
                    on_message(message);
                }
            });
        });
        let cancel_handler = RcBlock::new(move |error: *mut c_void| {
            run_handler(|| on_cancel(unsafe { XpcRichError::from_borrowed(error.cast()) }));
        });
        unsafe {
            xpc_session_set_incoming_message_handler(session.raw(), RcBlock::as_ptr(&message_handler).cast());
            xpc_session_set_cancel_handler(session.raw(), RcBlock::as_ptr(&cancel_handler).cast());
        }
        let mut error = ptr::null_mut();
        if !unsafe { xpc_session_activate(session.raw(), &raw mut error) } {
            return Err(unsafe { XpcRichError::from_owned(error) });
        }
        Ok(session)
    }

    pub fn send(&self, message: &XpcDictionary) -> Result<(), XpcRichError> {
        let error = unsafe { xpc_session_send_message(self.raw(), message.as_object().as_raw()) };
        if error.is_null() {
            Ok(())
        } else {
            Err(unsafe { XpcRichError::from_owned(error) })
        }
    }

    pub fn send_with_reply_sync(&self, message: &XpcDictionary) -> Result<XpcDictionary, XpcRichError> {
        let mut error = ptr::null_mut();
        let reply = unsafe {
            xpc_session_send_message_with_reply_sync(self.raw(), message.as_object().as_raw(), &raw mut error)
        };
        if reply.is_null() {
            return Err(unsafe { XpcRichError::from_owned(error) });
        }
        XpcDictionary::from_object(unsafe { XpcObject::from_owned(reply) })
            .ok_or_else(|| XpcRichError::rejected("the reply is not a dictionary"))
    }

    pub fn cancel(&self) {
        unsafe { xpc_session_cancel(self.raw()) };
    }

    fn raw(&self) -> xpc_session_t {
        self.object.as_raw().cast()
    }
}

impl Drop for XpcSession {
    fn drop(&mut self) {
        self.cancel();
    }
}
