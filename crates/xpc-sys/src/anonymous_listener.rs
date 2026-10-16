use std::ptr;

use crate::sys::xpc_connection_create;
use crate::xpc_connection::XpcConnection;
use crate::xpc_endpoint::XpcEndpoint;
use crate::xpc_event::XpcEvent;

#[derive(Debug)]
pub struct AnonymousListener {
    connection: XpcConnection,
}

impl AnonymousListener {
    pub fn new(on_peer: impl Fn(XpcConnection) + Send + Sync + 'static) -> Self {
        let connection = unsafe { XpcConnection::from_owned(xpc_connection_create(ptr::null(), ptr::null_mut())) };
        connection.set_event_handler(move |event| {
            if let XpcEvent::Peer(peer) = event {
                on_peer(peer);
            }
        });
        connection.activate();
        Self { connection }
    }

    pub fn endpoint(&self) -> XpcEndpoint {
        XpcEndpoint::of(&self.connection)
    }
}

impl Drop for AnonymousListener {
    fn drop(&mut self) {
        self.connection.cancel();
    }
}
