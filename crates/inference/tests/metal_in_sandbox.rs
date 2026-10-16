mod metal_resources;
mod test_host;

use std::fs::File;
use std::path::Path;

use domovoi_inference_proto::{GenerateRequest, Message, ModelClass, Sampling};
use test_host::{ServiceClient, TestHost, scenario};

const TINY_RANDOM: &str = "../../assets/test/tiny-random.gguf";
const CONTEXT_LENGTH: u32 = 2048;
const MAX_TOKENS: u32 = 16;

#[test]
#[ignore = "drives Metal inside the App Sandbox on the nightly self-hosted machine"]
fn the_sandboxed_service_generates_on_the_metal_device() {
    let host = TestHost::build(&[("DOMOVOI_INFERENCE_BACKEND", "llama")]);
    let output = host.run("metal", &[]);
    assert!(output.contains("scenario metal passed"), "{output}");
}

#[test]
#[ignore = "runs inside the temporary TestHost.app"]
fn host_scenario() {
    let (name, service) = scenario();
    assert_eq!(name, "metal", "unknown scenario {name}");
    let weights = File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(TINY_RANDOM)).expect("the tiny model opens");
    let client = ServiceClient::connect(&service);
    let loaded = client.request(&Message::Load {
        weights: weights.into(),
        class: ModelClass::Compact,
        context_length: CONTEXT_LENGTH,
    });
    assert!(matches!(loaded, Ok(Message::Ready)), "{loaded:?}");
    let generated = client.request(&Message::Generate(GenerateRequest {
        prompt: "hello".to_owned(),
        grammar: None,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
        max_tokens: MAX_TOKENS,
    }));
    match generated {
        Ok(Message::Done(stats)) => assert!(stats.generated_tokens > 0, "{stats:?}"),
        other => panic!("expected a finished generation, got {other:?}"),
    }
    println!("scenario {name} passed");
}
