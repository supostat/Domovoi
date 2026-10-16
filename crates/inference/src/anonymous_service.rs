use std::sync::Arc;
use std::time::Duration;

use domovoi_xpc_sys::{AnonymousListener, XpcEndpoint, XpcEvent};

use crate::inference_backend::InferenceBackend;
use crate::inference_service::InferenceService;
use crate::message_sink::MessageSink;

#[derive(Debug)]
pub struct AnonymousService {
    listener: AnonymousListener,
    service: Arc<InferenceService>,
}

impl AnonymousService {
    pub fn start(backend: Box<dyn InferenceBackend>, idle_unload: Option<Duration>) -> Self {
        let service = Arc::new(InferenceService::start(backend, idle_unload));
        let handler_service = Arc::clone(&service);
        let listener = AnonymousListener::new(move |peer| {
            let service = Arc::clone(&handler_service);
            let sink: Arc<dyn MessageSink> = Arc::new(peer.clone());
            peer.set_event_handler(move |event| {
                if let XpcEvent::Message(message) = event {
                    service.handle(message, Arc::clone(&sink));
                }
            });
            peer.activate();
        });
        Self { listener, service }
    }

    pub fn endpoint(&self) -> XpcEndpoint {
        self.listener.endpoint()
    }

    pub fn service(&self) -> &InferenceService {
        &self.service
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::process;
    use std::sync::mpsc;

    use domovoi_inference_proto::{GenerateRequest, Message, ModelClass, Sampling, StopReason};
    use domovoi_testkit::{TempTree, test_timeout};
    use domovoi_xpc_sys::{XpcConnection, XpcEvent};

    use super::AnonymousService;
    use crate::fake_backend::FakeBackend;

    fn request(client: &XpcConnection, message: &Message) -> Message {
        match client.send_with_reply_sync(&message.encode().expect("the request encodes")) {
            XpcEvent::Message(reply) => Message::decode(&reply).expect("the reply decodes"),
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn a_client_drives_the_fake_backend_through_the_anonymous_endpoint() {
        let service = AnonymousService::start(Box::new(FakeBackend::default()), None);
        let (token_sender, tokens) = mpsc::channel();
        let client = service.endpoint().connect();
        client.set_event_handler(move |event| {
            if let XpcEvent::Message(message) = event
                && let Ok(Message::Token { text }) = Message::decode(&message)
            {
                let _ = token_sender.send(text);
            }
        });
        client.activate();

        let own = i32::try_from(process::id()).expect("a pid fits in i32");
        let handshake = request(&client, &Message::Handshake { pid: own });
        assert!(
            matches!(handshake, Message::Handshake { pid } if pid == own),
            "{handshake:?}"
        );

        let workspace = TempTree::new().expect("a temp tree is created");
        let weights = workspace.path().join("weights.gguf");
        fs::write(&weights, b"GGUF").expect("the weights are written");
        let loaded = request(
            &client,
            &Message::Load {
                weights: File::open(&weights).expect("the weights open").into(),
                class: ModelClass::Compact,
                context_length: 512,
            },
        );
        assert!(matches!(loaded, Message::Ready), "{loaded:?}");

        let generated = request(
            &client,
            &Message::Generate(GenerateRequest {
                prompt: "token Домовой".to_owned(),
                grammar: None,
                sampling: Sampling {
                    temperature: 0.0,
                    top_k: 0,
                    top_p: 1.0,
                    seed: 0,
                },
                max_tokens: 8,
            }),
        );
        assert!(
            matches!(&generated, Message::Done(stats) if stats.generated_tokens == 1 && stats.stop == StopReason::EndOfGeneration),
            "{generated:?}"
        );
        assert_eq!(
            tokens.recv_timeout(test_timeout()).expect("the token arrives"),
            "Домовой"
        );
    }
}
