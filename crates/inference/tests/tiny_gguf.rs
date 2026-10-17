use std::env;
use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::sync::Once;

use domovoi_inference::{CancelFlag, DeviceProfile, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::{ErrorCode, GenerateRequest, GenerationStats, ModelClass, Sampling, StopReason};

const TINY_RANDOM: &str = "../../assets/test/tiny-random.gguf";
const CONTEXT_LENGTH: u32 = 2048;
const CANCEL_AT: f32 = 0.5;
const JSON_GRAMMAR: &str = r#"root ::= "{" space (pair ("," space pair){0,3})? "}" space
pair ::= "\"" [a-z]{1,8} "\"" space ":" space [0-9]{1,4} space
space ::= " "?"#;
static CPU_ONLY: Once = Once::new();

fn cpu_backend() -> LlamaBackend {
    CPU_ONLY.call_once(|| unsafe { env::set_var("GGML_METAL_DEVICES", "0") });
    LlamaBackend::new(DeviceProfile::Cpu)
}

fn weights() -> OwnedFd {
    File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(TINY_RANDOM))
        .expect("the tiny model opens")
        .into()
}

fn loaded() -> LlamaBackend {
    let mut backend = cpu_backend();
    backend
        .load(weights(), ModelClass::Compact, CONTEXT_LENGTH, &CancelFlag::default())
        .expect("the tiny model loads from a descriptor");
    backend
}

fn request(prompt: &str, grammar: Option<&str>, max_tokens: u32) -> GenerateRequest {
    GenerateRequest {
        prompt: prompt.to_owned(),
        grammar: grammar.map(str::to_owned),
        sampling: Sampling {
            temperature: 0.0,
            top_k: 40,
            top_p: 0.95,
            seed: 7,
        },
        max_tokens,
    }
}

fn generate(backend: &mut LlamaBackend, request: &GenerateRequest) -> (String, GenerationStats) {
    let mut text = String::new();
    let stats = backend
        .generate(request, &CancelFlag::default(), &mut |piece| text.push_str(piece))
        .expect("generation succeeds");
    (text, stats)
}

#[test]
fn the_model_loads_from_a_descriptor_and_unloads() {
    let mut backend = loaded();
    assert!(backend.is_loaded());
    backend.unload();
    assert!(!backend.is_loaded());
    let refused = backend
        .generate(&request("hi", None, 4), &CancelFlag::default(), &mut |_| {})
        .expect_err("an unloaded backend refuses to generate");
    assert_eq!(refused.code, ErrorCode::ModelNotLoaded);
}

#[test]
fn loading_stops_when_the_progress_callback_declines() {
    let mut backend = cpu_backend();
    let mut furthest: f32 = 0.0;
    let refused = backend
        .load_with_progress(weights(), ModelClass::Compact, CONTEXT_LENGTH, &mut |fraction| {
            furthest = furthest.max(fraction);
            fraction <= CANCEL_AT
        })
        .expect_err("a declined load yields no model");
    assert_eq!(refused.code, ErrorCode::LoadCancelled);
    assert!(furthest > CANCEL_AT, "progress reached {furthest}");
    assert!(!backend.is_loaded());
}

#[test]
fn tokens_stream_deterministically_at_zero_temperature() {
    let mut backend = loaded();
    let mut pieces = Vec::new();
    let stats = backend
        .generate(
            &request("Once upon a time", None, 24),
            &CancelFlag::default(),
            &mut |piece| {
                pieces.push(piece.to_owned());
            },
        )
        .expect("generation succeeds");
    assert!(!pieces.is_empty());
    assert!(stats.generated_tokens > 0);
    assert!(stats.prompt_tokens > 0);
    let (again, again_stats) = generate(&mut backend, &request("Once upon a time", None, 24));
    assert_eq!(pieces.concat(), again);
    assert_eq!(stats.generated_tokens, again_stats.generated_tokens);
    assert_eq!(stats.stop, again_stats.stop);
}

#[test]
fn the_abort_callback_stops_decoding_after_cancellation() {
    let mut backend = loaded();
    let cancel = CancelFlag::default();
    let stats = backend
        .generate(&request("Once upon a time", None, 64), &cancel, &mut |_| cancel.raise())
        .expect("a cancelled generation still reports");
    assert_eq!(stats.stop, StopReason::Cancelled);
    assert_eq!(stats.generated_tokens, 1);
}

#[test]
fn the_grammar_keeps_a_random_model_inside_json() {
    let mut backend = loaded();
    let (text, stats) = generate(&mut backend, &request("JSON:", Some(JSON_GRAMMAR), 256));
    assert_eq!(stats.stop, StopReason::EndOfGeneration, "{text}");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    assert!(value.is_object());
}

#[test]
fn the_token_count_matches_the_prompt_the_model_decodes() {
    let mut backend = loaded();
    let prompt = "<|im_start|>user\nСколько места?<|im_end|>\n";
    let counted = backend.count_tokens(prompt).expect("the loaded model counts tokens");
    let (_, stats) = generate(&mut backend, &request(prompt, None, 1));
    assert_eq!(counted, stats.prompt_tokens);
    backend.unload();
    assert_eq!(
        backend.count_tokens(prompt).map_err(|error| error.code),
        Err(ErrorCode::ModelNotLoaded)
    );
}
