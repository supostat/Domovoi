use std::sync::Arc;
use std::sync::mpsc::Sender;

use domovoi_inference_proto::Message;
use domovoi_xpc_sys::{XpcConnection, XpcEvent, XpcSession};

use crate::service_address::ServiceAddress;

#[derive(Clone, Debug)]
pub(crate) enum ServiceLink {
    Session(Arc<XpcSession>),
    Connection(Arc<XpcConnection>),
}

impl ServiceLink {
    pub(crate) fn open(address: &ServiceAddress, tokens: Sender<String>) -> Result<Self, String> {
        let forward = move |message| {
            if let Ok(Message::Token { text }) = Message::decode(&message) {
                let _ = tokens.send(text);
            }
        };
        match address {
            ServiceAddress::Bundled(name) => XpcSession::connect_service(name, forward, |_| {})
                .map(|session| Self::Session(Arc::new(session)))
                .map_err(|error| error.description),
            ServiceAddress::Endpoint(endpoint) => {
                let connection = XpcConnection::from_endpoint(endpoint);
                connection.set_event_handler(move |event| {
                    if let XpcEvent::Message(message) = event {
                        forward(message);
                    }
                });
                connection.activate();
                Ok(Self::Connection(Arc::new(connection)))
            }
        }
    }

    pub(crate) fn request(&self, message: &Message) -> Result<Message, String> {
        let encoded = message.encode().map_err(|error| error.to_string())?;
        let reply = match self {
            Self::Session(session) => session
                .send_with_reply_sync(&encoded)
                .map_err(|error| error.description)?,
            Self::Connection(connection) => match connection.send_with_reply_sync(&encoded) {
                XpcEvent::Message(reply) => reply,
                other => return Err(format!("the service did not reply: {other:?}")),
            },
        };
        Message::decode(&reply).map_err(|error| error.to_string())
    }

    pub(crate) fn close(&self) {
        match self {
            Self::Session(session) => session.cancel(),
            Self::Connection(connection) => connection.cancel(),
        }
    }
}
