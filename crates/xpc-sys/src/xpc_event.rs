use std::ptr;

use crate::sys::{
    _xpc_error_connection_interrupted, _xpc_error_connection_invalid, _xpc_error_peer_code_signing_requirement,
    _xpc_error_termination_imminent, _xpc_type_connection, _xpc_type_dictionary, _xpc_type_error, xpc_object_t,
};
use crate::xpc_connection::XpcConnection;
use crate::xpc_dictionary::XpcDictionary;
use crate::xpc_object::XpcObject;

#[derive(Debug)]
pub enum XpcEvent {
    Message(XpcDictionary),
    Peer(XpcConnection),
    Interrupted,
    Invalid,
    TerminationImminent,
    PeerRequirementNotMet,
    Unexpected(XpcObject),
}

impl XpcEvent {
    pub(crate) unsafe fn from_borrowed(raw: xpc_object_t) -> Self {
        unsafe { Self::from_object(XpcObject::from_borrowed(raw)) }
    }

    pub(crate) unsafe fn from_owned(raw: xpc_object_t) -> Self {
        unsafe { Self::from_object(XpcObject::from_owned(raw)) }
    }

    fn from_object(object: XpcObject) -> Self {
        let raw = object.as_raw().cast_const();
        if object.has_type(unsafe { &_xpc_type_error }) {
            let errors = [
                (&raw const _xpc_error_connection_interrupted, Self::Interrupted),
                (&raw const _xpc_error_connection_invalid, Self::Invalid),
                (&raw const _xpc_error_termination_imminent, Self::TerminationImminent),
                (
                    &raw const _xpc_error_peer_code_signing_requirement,
                    Self::PeerRequirementNotMet,
                ),
            ];
            return errors
                .into_iter()
                .find(|(known, _)| ptr::eq(known.cast(), raw))
                .map_or(Self::Unexpected(object), |(_, event)| event);
        }
        if object.has_type(unsafe { &_xpc_type_connection }) {
            return Self::Peer(XpcConnection::from_object(object));
        }
        if object.has_type(unsafe { &_xpc_type_dictionary }) {
            return Self::Message(XpcDictionary::from_object(object).expect("the object is a dictionary"));
        }
        Self::Unexpected(object)
    }
}
