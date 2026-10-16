use std::ffi::CStr;
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::AsFd;
use std::sync::mpsc;

use domovoi_testkit::{TempTree, test_timeout};
use domovoi_xpc_sys::{AnonymousListener, XpcConnection, XpcDictionary, XpcEvent};

const ECHO: &CStr = c"echo";
const FILE: &CStr = c"file";
const CONTENT: &CStr = c"content";
const SIGNED: &CStr = c"signed";
const UNSIGNED: &CStr = c"unsigned";
const FLAG: &CStr = c"flag";
const RATIO: &CStr = c"ratio";
const NAME: &CStr = c"name";
const PAYLOAD: &CStr = c"payload";
const NESTED: &CStr = c"nested";
const MISSING: &CStr = c"missing";
const WEIGHTS: &[u8] = b"pretend weights";

fn echo_listener() -> AnonymousListener {
    AnonymousListener::new(|peer| {
        let replier = peer.clone();
        peer.set_event_handler(move |event| {
            if let XpcEvent::Message(message) = event {
                let mut reply = message.reply().expect("the client waits for a reply");
                reply.set_dictionary(ECHO, &message);
                if let Some(descriptor) = message.dup_fd(FILE) {
                    let mut content = Vec::new();
                    File::from(descriptor)
                        .read_to_end(&mut content)
                        .expect("the passed file reads");
                    reply.set_bytes(CONTENT, &content);
                }
                replier.send(&reply);
            }
        });
        peer.activate();
    })
}

fn connect(listener: &AnonymousListener) -> XpcConnection {
    let client = listener.endpoint().connect();
    client.set_event_handler(|_| {});
    client.activate();
    client
}

fn reply(client: &XpcConnection, message: &XpcDictionary) -> XpcDictionary {
    match client.send_with_reply_sync(message) {
        XpcEvent::Message(reply) => reply,
        other => panic!("expected a reply, got {other:?}"),
    }
}

#[test]
fn every_field_type_echoes_through_an_anonymous_connection() {
    let listener = echo_listener();
    let client = connect(&listener);
    let mut nested = XpcDictionary::new();
    nested.set_string(NAME, c"вложенный");
    let mut message = XpcDictionary::new();
    message.set_i64(SIGNED, -42);
    message.set_u64(UNSIGNED, u64::MAX);
    message.set_bool(FLAG, true);
    message.set_f64(RATIO, 0.1);
    message.set_string(NAME, c"Домовой");
    message.set_bytes(PAYLOAD, b"\0binary\xff");
    message.set_dictionary(NESTED, &nested);

    let echoed = reply(&client, &message)
        .dictionary(ECHO)
        .expect("the reply carries the echo");

    assert_eq!(echoed.i64(SIGNED), Some(-42));
    assert_eq!(echoed.u64(UNSIGNED), Some(u64::MAX));
    assert_eq!(echoed.bool(FLAG), Some(true));
    assert_eq!(echoed.f64(RATIO).map(f64::to_bits), Some(0.1_f64.to_bits()));
    assert_eq!(echoed.string(NAME), Some(c"Домовой"));
    assert_eq!(echoed.bytes(PAYLOAD), Some(&b"\0binary\xff"[..]));
    assert_eq!(
        echoed
            .dictionary(NESTED)
            .and_then(|inner| inner.string(NAME).map(CStr::to_owned)),
        Some(c"вложенный".to_owned())
    );
    assert_eq!(echoed.i64(MISSING), None);
    assert_eq!(echoed.i64(NAME), None);
    assert!(!echoed.contains(MISSING));
    client.cancel();
}

#[test]
fn a_file_descriptor_crosses_the_connection() {
    let workspace = TempTree::new().expect("a temp tree is created");
    let path = workspace.path().join("weights.gguf");
    fs::write(&path, WEIGHTS).expect("the weights are written");
    let listener = echo_listener();
    let client = connect(&listener);
    let file = File::open(&path).expect("the weights open");
    let mut message = XpcDictionary::new();
    message.set_fd(FILE, file.as_fd()).expect("the descriptor wraps");

    assert_eq!(reply(&client, &message).bytes(CONTENT), Some(WEIGHTS));
    client.cancel();
}

#[test]
fn a_peer_cancelled_by_the_listener_interrupts_the_client() {
    let listener = AnonymousListener::new(|peer| {
        let cancelled = peer.clone();
        peer.set_event_handler(move |event| {
            if matches!(event, XpcEvent::Message(_)) {
                cancelled.cancel();
            }
        });
        peer.activate();
    });
    let client = listener.endpoint().connect();
    let (sender, receiver) = mpsc::channel();
    client.set_event_handler(move |event| {
        let _ = sender.send(matches!(event, XpcEvent::Interrupted));
    });
    client.activate();

    let outcome = client.send_with_reply_sync(&XpcDictionary::new());
    assert!(matches!(outcome, XpcEvent::Interrupted), "{outcome:?}");
    assert!(
        receiver
            .recv_timeout(test_timeout())
            .expect("the client hears about the cancellation"),
        "the first client event is an interruption"
    );
    client.cancel();
}
