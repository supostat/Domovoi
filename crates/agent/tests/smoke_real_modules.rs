mod fixture;

use std::env;
use std::fs::File;
use std::path::Path;
use std::process::Command;

use domovoi_agent::{Agent, AgentLimits};
use domovoi_grammar::{ChatTemplate, PromptFamily};
use domovoi_inference::{CancelFlag, DeviceProfile, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::{ModelClass, Sampling};
use fixture::Fixture;

const METAL_DEVICES: &str = "GGML_METAL_DEVICES";
const SELF_NAME: &str = "a_real_index_rules_grammar_and_llama_answer_by_the_schema_within_the_step_limit";
const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const CONTEXT_LENGTH: u32 = 65_536;
const TOKENS_PER_STEP: u32 = 32_768;
const STEPS: u32 = 3;

#[test]
fn a_real_index_rules_grammar_and_llama_answer_by_the_schema_within_the_step_limit() {
    if env::var_os(METAL_DEVICES).is_none() {
        let status = Command::new(env::current_exe().expect("the test binary is known"))
            .args(["--exact", SELF_NAME, "--nocapture", "--test-threads=1"])
            .env(METAL_DEVICES, "0")
            .status()
            .expect("the CPU-only run starts");
        assert!(status.success(), "the CPU-only run failed with {status}");
        return;
    }
    let fixture = Fixture::with_folders(&[]);
    let mut backend = LlamaBackend::new(DeviceProfile::Cpu);
    let weights = File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(TINY_MODEL)).expect("the tiny model opens");
    backend
        .load(
            weights.into(),
            ModelClass::Compact,
            CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the tiny model loads on the CPU");
    let limits = AgentLimits {
        max_steps: STEPS,
        max_tokens_per_step: TOKENS_PER_STEP,
        context_length: CONTEXT_LENGTH,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
    };
    let template = ChatTemplate {
        family: PromptFamily::ChatMl,
        empty_think_before_answer: true,
        empty_think_in_tool_steps: false,
    };
    let mut observed = 0;
    let answer = Agent::new(&mut backend, template, limits)
        .calling_a_tool_first()
        .ask(
            &fixture.context(),
            "Что занимает больше всего места?",
            &CancelFlag::default(),
            &mut |_| {
                observed += 1;
            },
        )
        .expect("a random model under the grammar still answers by the schema");
    assert!(
        !answer.steps.is_empty(),
        "the first step calls a tool on the real index"
    );
    assert!(answer.steps.len() < STEPS as usize);
    assert_eq!(observed, answer.steps.len());
    assert!(!answer.text.is_empty());
    for hypothesis in &answer.hypotheses {
        assert!(fixture.index.find(&hypothesis.path).is_some());
    }
}
