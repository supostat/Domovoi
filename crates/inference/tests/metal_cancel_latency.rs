mod metal_resources;
mod nightly_model;

use std::thread;
use std::time::{Duration, Instant};

use domovoi_inference::{CancelFlag, DeviceProfile, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::{GenerateRequest, ModelClass, Sampling, StopReason};

const SENTENCE: &str = "The folder holds project sources, build caches and old disk images. ";
const REPETITIONS: usize = 300;
const WARM_UP_REPETITIONS: usize = 40;
const CANCEL_AFTER: Duration = Duration::from_millis(200);
const LATENCY_LIMIT: Duration = Duration::from_millis(300);
const MAX_TOKENS: u32 = 8;

fn request(repetitions: usize) -> GenerateRequest {
    GenerateRequest {
        prompt: SENTENCE.repeat(repetitions),
        grammar: None,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
        max_tokens: MAX_TOKENS,
    }
}

#[test]
#[ignore = "decodes a long prompt with the compact catalog model on the nightly self-hosted machine"]
fn cancellation_stops_metal_prompt_decoding_within_the_latency_limit() {
    metal_resources::place_metallibs_beside_the_test_binary();
    let mut backend = LlamaBackend::new(DeviceProfile::Gpu);
    backend
        .load(
            nightly_model::compact_model(),
            ModelClass::Compact,
            nightly_model::COMPACT_CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the compact model loads on Metal");
    backend
        .generate(&request(WARM_UP_REPETITIONS), &CancelFlag::default(), &mut |_| {})
        .expect("the warm-up generation builds the Metal pipelines");
    let cancel = CancelFlag::default();
    let remote_cancel = cancel.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(CANCEL_AFTER);
        remote_cancel.raise();
        Instant::now()
    });
    let stats = backend
        .generate(&request(REPETITIONS), &cancel, &mut |_| {})
        .expect("the cancelled generation returns its statistics");
    let returned = Instant::now();
    let raised = canceller.join().expect("the canceller finishes");
    assert_eq!(stats.stop, StopReason::Cancelled, "{stats:?}");
    assert_eq!(
        stats.generated_tokens, 0,
        "the prompt was still decoding when cancelled"
    );
    let latency = returned.duration_since(raised);
    eprintln!("decoding stopped {latency:?} after cancellation on Metal");
    assert!(
        latency <= LATENCY_LIMIT,
        "decoding stopped {latency:?} after cancellation, the limit is {LATENCY_LIMIT:?}"
    );
}
