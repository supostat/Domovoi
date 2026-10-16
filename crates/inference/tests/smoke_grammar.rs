use std::env;
use std::fs::File;
use std::path::Path;

use domovoi_grammar::{ChatMessage, ChatTemplate, ModelResponse, PromptFamily, ResponseSchema, system_prompt};
use domovoi_inference::{CancelFlag, DeviceProfile, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::{GenerateRequest, ModelClass, Sampling, StopReason};

const TINY_RANDOM: &str = "../../assets/test/tiny-random.gguf";
const CONTEXT_LENGTH: u32 = 16_384;
const MAX_TOKENS: u32 = 12_000;

#[test]
fn the_response_grammar_turns_a_random_model_into_a_valid_response() {
    unsafe { env::set_var("GGML_METAL_DEVICES", "0") };
    let mut backend = LlamaBackend::new(DeviceProfile::Cpu);
    let weights = File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(TINY_RANDOM)).expect("the tiny model opens");
    backend
        .load(
            weights.into(),
            ModelClass::Compact,
            CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the tiny model loads");
    let schema = ResponseSchema::answers_and_tool_calls();
    let template = ChatTemplate {
        family: PromptFamily::ChatMl,
        empty_think_before_answer: true,
        empty_think_in_tool_steps: false,
    };
    let prompt = template.render(
        &system_prompt("Ты — Домовой.", &schema),
        &[ChatMessage::user("Что занимает место?")],
    );
    let mut text = String::new();
    let stats = backend
        .generate(
            &GenerateRequest {
                prompt,
                grammar: Some(schema.grammar().as_str().to_owned()),
                sampling: Sampling {
                    temperature: 0.0,
                    top_k: 40,
                    top_p: 0.95,
                    seed: 1,
                },
                max_tokens: MAX_TOKENS,
            },
            &CancelFlag::default(),
            &mut |piece| text.push_str(piece),
        )
        .expect("generation succeeds");

    assert_eq!(stats.stop, StopReason::EndOfGeneration, "{text}");
    let response = schema.parse(&text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
    assert!(matches!(response, ModelResponse::ToolCall(_) | ModelResponse::Final(_)));
}
