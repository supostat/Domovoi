use domovoi_xpc_sys::{PeerSession, XpcConnection, XpcDictionary};

pub trait MessageSink: Send + Sync {
    fn deliver(&self, message: &XpcDictionary);
}

impl MessageSink for PeerSession {
    fn deliver(&self, message: &XpcDictionary) {
        if let Err(error) = self.send(message) {
            eprintln!("domovoi-inference: the peer did not receive a message: {error}");
        }
    }
}

impl MessageSink for XpcConnection {
    fn deliver(&self, message: &XpcDictionary) {
        self.send(message);
    }
}
