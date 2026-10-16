use std::ffi::CStr;
use std::fs::{self, File};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

use domovoi_inference_proto::{
    DecodeError, ErrorCode, GenerateRequest, GenerationStats, Message, ModelClass, PROTOCOL_VERSION, Sampling,
    StopReason, TimeoutError, Timeouts,
};
use domovoi_testkit::TempTree;
use domovoi_xpc_sys::{AnonymousListener, XpcConnection, XpcDictionary, XpcEvent};

const ECHO: &CStr = c"echo";
const GRAMMAR: &str = "root ::= \"{\" \"}\"";

fn inode(file: &OwnedFd) -> u64 {
    File::from(file.try_clone().expect("the descriptor duplicates"))
        .metadata()
        .expect("the descriptor is inspected")
        .ino()
}

fn messages(weights: &File) -> Vec<Message> {
    vec![
        Message::Handshake { pid: 4242 },
        Message::Load {
            weights: weights.as_fd().try_clone_to_owned().expect("the descriptor duplicates"),
            class: ModelClass::Standard,
            context_length: 16_384,
        },
        Message::Unload,
        Message::Generate(GenerateRequest {
            prompt: "<|im_start|>user\nПочему диск полон?\0<|im_end|>\n".to_owned(),
            grammar: Some(GRAMMAR.to_owned()),
            sampling: Sampling {
                temperature: 0.0,
                top_k: 40,
                top_p: 0.95,
                seed: u32::MAX,
            },
            max_tokens: 256,
        }),
        Message::Generate(GenerateRequest {
            prompt: "без грамматики".to_owned(),
            grammar: None,
            sampling: Sampling {
                temperature: 0.7,
                top_k: 0,
                top_p: 1.0,
                seed: 0,
            },
            max_tokens: 1,
        }),
        Message::Token {
            text: "Кэш".to_owned()
        },
        Message::Done(GenerationStats {
            prompt_tokens: 1200,
            generated_tokens: 87,
            prompt_duration: Duration::new(1, 234_567_891),
            generation_duration: Duration::from_nanos(u64::MAX),
            stop: StopReason::EndOfGeneration,
        }),
        Message::Cancel,
        Message::Error {
            code: ErrorCode::InvalidGrammar,
            description: "grammar root is missing".to_owned(),
        },
    ]
}

fn assert_same(sent: &Message, received: &Message) {
    match (sent, received) {
        (
            Message::Load {
                weights: sent_weights,
                class: sent_class,
                context_length: sent_length,
            },
            Message::Load {
                weights: received_weights,
                class: received_class,
                context_length: received_length,
            },
        ) => {
            assert_eq!(inode(sent_weights), inode(received_weights));
            assert_eq!((sent_class, sent_length), (received_class, received_length));
        }
        (Message::Generate(sent), Message::Generate(received)) => assert_eq!(sent, received),
        (Message::Done(sent), Message::Done(received)) => assert_eq!(sent, received),
        (sent, received) => assert_eq!(format!("{sent:?}"), format!("{received:?}")),
    }
}

fn weights_file(workspace: &TempTree) -> File {
    let path = workspace.path().join("model.gguf");
    fs::write(&path, b"GGUF").expect("the weights are written");
    File::open(path).expect("the weights open")
}

#[test]
fn every_message_survives_encoding_and_decoding() {
    let workspace = TempTree::new().expect("a temp tree is created");
    let weights = weights_file(&workspace);
    for message in messages(&weights) {
        let encoded = message.encode().expect("the message encodes");
        let decoded = Message::decode(&encoded).unwrap_or_else(|error| panic!("{message:?}: {error}"));
        assert_same(&message, &decoded);
    }
}

#[test]
fn every_message_crosses_an_anonymous_connection() {
    let workspace = TempTree::new().expect("a temp tree is created");
    let weights = weights_file(&workspace);
    let listener = AnonymousListener::new(|peer| {
        let replier = peer.clone();
        peer.set_event_handler(move |event| {
            if let XpcEvent::Message(message) = event {
                let mut reply = message.reply().expect("the client waits for a reply");
                reply.set_dictionary(ECHO, &message);
                replier.send(&reply);
            }
        });
        peer.activate();
    });
    let client = XpcConnection::from_endpoint(&listener.endpoint());
    client.set_event_handler(|_| {});
    client.activate();
    for message in messages(&weights) {
        let XpcEvent::Message(reply) = client.send_with_reply_sync(&message.encode().expect("the message encodes"))
        else {
            panic!("the echo replies");
        };
        let echoed = reply.dictionary(ECHO).expect("the reply carries the message");
        assert_same(&message, &Message::decode(&echoed).expect("the echoed message decodes"));
    }
    client.cancel();
}

#[test]
fn unknown_versions_kinds_and_malformed_fields_are_refused() {
    let unload = Message::Unload.encode().expect("the message encodes");

    let mut future = unload.clone();
    future.set_u64(c"version", PROTOCOL_VERSION + 1);
    assert_eq!(
        Message::decode(&future).expect_err("a newer version is refused"),
        DecodeError::UnsupportedVersion(PROTOCOL_VERSION + 1)
    );

    let mut unversioned = XpcDictionary::new();
    unversioned.set_string(c"kind", c"unload");
    assert_eq!(
        Message::decode(&unversioned).expect_err("a message needs a version"),
        DecodeError::MissingVersion
    );

    let mut unknown = unload.clone();
    unknown.set_string(c"kind", c"reboot");
    assert_eq!(
        Message::decode(&unknown).expect_err("an unknown kind is refused"),
        DecodeError::UnknownKind("reboot".to_owned())
    );

    let mut handshake = unload.clone();
    handshake.set_string(c"kind", c"handshake");
    assert_eq!(
        Message::decode(&handshake).expect_err("a handshake needs a pid"),
        DecodeError::MissingField(c"pid")
    );
    handshake.set_string(c"pid", c"4242");
    assert_eq!(
        Message::decode(&handshake).expect_err("a pid is an integer"),
        DecodeError::InvalidField(c"pid")
    );

    let mut token = unload.clone();
    token.set_string(c"kind", c"token");
    token.set_bytes(c"text", b"\xff\xfe");
    assert_eq!(
        Message::decode(&token).expect_err("token text is UTF-8"),
        DecodeError::InvalidField(c"text")
    );

    let mut error = unload;
    error.set_string(c"kind", c"error");
    error.set_string(c"code", c"meltdown");
    error.set_bytes(c"description", b"x");
    assert_eq!(
        Message::decode(&error).expect_err("an unknown error code is refused"),
        DecodeError::InvalidField(c"code")
    );
}

#[test]
fn timeouts_come_from_the_environment_with_defaults() {
    assert_eq!(
        Timeouts::from_variables(|_| None).expect("defaults apply"),
        Timeouts {
            handshake: Duration::from_secs(5),
            load: Duration::from_secs(120),
            stall: Duration::from_secs(30),
        }
    );
    let overridden =
        Timeouts::from_variables(|name| (name == "DOMOVOI_XPC_STALL_TIMEOUT_MS").then(|| "1500".to_owned()))
            .expect("an override applies");
    assert_eq!(overridden.stall, Duration::from_millis(1500));
    assert_eq!(
        Timeouts::from_variables(|name| (name == "DOMOVOI_XPC_LOAD_TIMEOUT_MS").then(|| "0".to_owned())),
        Err(TimeoutError {
            variable: "DOMOVOI_XPC_LOAD_TIMEOUT_MS",
            value: "0".to_owned(),
        })
    );
}
