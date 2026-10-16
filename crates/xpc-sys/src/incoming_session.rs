use std::ffi::{CStr, c_void};
use std::ptr::NonNull;

use block2::RcBlock;

use crate::handler_boundary::run_handler;
use crate::peer_session::PeerSession;
use crate::sys::{xpc_listener_reject_peer, xpc_object_t, xpc_retain, xpc_session_set_incoming_message_handler};
use crate::xpc_dictionary::XpcDictionary;
use crate::xpc_object::XpcObject;

const NOT_ACCEPTED: &CStr = c"the peer session was not accepted";

#[derive(Debug)]
pub struct IncomingSession {
    raw: Option<NonNull<c_void>>,
}

unsafe impl Send for IncomingSession {}

impl IncomingSession {
    pub(crate) unsafe fn from_borrowed(raw: *mut c_void) -> Self {
        Self {
            raw: Some(NonNull::new(unsafe { xpc_retain(raw) }).expect("XPC passed a peer session")),
        }
    }

    pub fn accept(mut self, on_message: impl Fn(&PeerSession, XpcDictionary) + Send + Sync + 'static) -> PeerSession {
        let raw = self.raw.take().expect("a pending session is accepted once");
        let peer = unsafe { PeerSession::from_owned(raw.as_ptr()) };
        let handler_peer = peer.clone();
        let handler = RcBlock::new(move |message: xpc_object_t| {
            run_handler(|| {
                let object = unsafe { XpcObject::from_borrowed(message) };
                if let Some(message) = XpcDictionary::from_object(object) {
                    on_message(&handler_peer, message);
                }
            });
        });
        unsafe { xpc_session_set_incoming_message_handler(raw.as_ptr().cast(), RcBlock::as_ptr(&handler).cast()) };
        peer
    }

    pub fn reject(mut self, reason: &CStr) {
        let raw = self.raw.take().expect("a pending session is rejected once");
        unsafe { reject_and_release(raw, reason) };
    }
}

impl Drop for IncomingSession {
    fn drop(&mut self) {
        if let Some(raw) = self.raw.take() {
            unsafe { reject_and_release(raw, NOT_ACCEPTED) };
        }
    }
}

unsafe fn reject_and_release(raw: NonNull<c_void>, reason: &CStr) {
    unsafe {
        xpc_listener_reject_peer(raw.as_ptr().cast(), reason.as_ptr());
        drop(XpcObject::from_owned(raw.as_ptr()));
    }
}
