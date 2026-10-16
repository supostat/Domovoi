use std::ffi::{CStr, CString, c_void};
use std::ptr::{self, NonNull};

use block2::RcBlock;

use crate::handler_boundary::run_handler;
use crate::incoming_session::IncomingSession;
use crate::sys::{
    XPC_LISTENER_CREATE_INACTIVE, xpc_listener_activate, xpc_listener_cancel, xpc_listener_create,
    xpc_listener_set_peer_code_signing_requirement, xpc_listener_t, xpc_release,
};
use crate::xpc_rich_error::XpcRichError;

pub const PEER_REQUIREMENT_ENFORCED: bool = !cfg!(feature = "adhoc");

#[derive(Debug)]
pub struct XpcListener {
    raw: NonNull<c_void>,
}

unsafe impl Send for XpcListener {}
unsafe impl Sync for XpcListener {}

impl XpcListener {
    pub fn new(
        service: &CStr,
        team_identifier: &str,
        on_session: impl Fn(IncomingSession) + Send + Sync + 'static,
    ) -> Result<Self, XpcRichError> {
        let handler = RcBlock::new(move |peer: *mut c_void| {
            run_handler(|| on_session(unsafe { IncomingSession::from_borrowed(peer) }));
        });
        let mut error = ptr::null_mut();
        let raw = unsafe {
            xpc_listener_create(
                service.as_ptr(),
                ptr::null_mut(),
                XPC_LISTENER_CREATE_INACTIVE,
                RcBlock::as_ptr(&handler).cast(),
                &raw mut error,
            )
        };
        let listener = Self {
            raw: NonNull::new(raw.cast()).ok_or_else(|| unsafe { XpcRichError::from_owned(error) })?,
        };
        listener.require_team(team_identifier)?;
        let mut error = ptr::null_mut();
        if !unsafe { xpc_listener_activate(listener.raw(), &raw mut error) } {
            return Err(unsafe { XpcRichError::from_owned(error) });
        }
        Ok(listener)
    }

    fn require_team(&self, team_identifier: &str) -> Result<(), XpcRichError> {
        if !PEER_REQUIREMENT_ENFORCED {
            return Ok(());
        }
        let requirement = CString::new(format!(
            "anchor apple generic and certificate leaf[subject.OU] = \"{team_identifier}\""
        ))
        .map_err(|_| XpcRichError::rejected("the team identifier contains NUL"))?;
        match unsafe { xpc_listener_set_peer_code_signing_requirement(self.raw(), requirement.as_ptr()) } {
            0 => Ok(()),
            status => Err(XpcRichError::rejected(&format!(
                "the peer code signing requirement was refused with {status}"
            ))),
        }
    }

    fn raw(&self) -> xpc_listener_t {
        self.raw.as_ptr().cast()
    }
}

impl Drop for XpcListener {
    fn drop(&mut self) {
        unsafe {
            xpc_listener_cancel(self.raw());
            xpc_release(self.raw.as_ptr());
        }
    }
}
