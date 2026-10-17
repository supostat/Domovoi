use domovoi_inference_proto::Sampling;

const DEFAULT_STEPS: u32 = 4;
const DEFAULT_TOKENS_PER_STEP: u32 = 512;
const GREEDY: Sampling = Sampling {
    temperature: 0.0,
    top_k: 0,
    top_p: 1.0,
    seed: 0,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentLimits {
    pub max_steps: u32,
    pub max_tokens_per_step: u32,
    pub context_length: u32,
    pub sampling: Sampling,
}

impl AgentLimits {
    pub fn for_context_length(context_length: u32) -> Self {
        Self {
            max_steps: DEFAULT_STEPS,
            max_tokens_per_step: DEFAULT_TOKENS_PER_STEP,
            context_length,
            sampling: GREEDY,
        }
    }
}
