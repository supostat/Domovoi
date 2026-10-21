use std::path::Path;
use std::time::Duration;

use domovoi_agent::{Agent, AgentContext, AgentError, AgentLimits, FocusContext};
use domovoi_grammar::{ChatMessage, ChatTemplate, ResponseSchema, system_prompt};
use domovoi_inference::{CancelFlag, InferenceBackend};
use domovoi_inference_proto::GenerateRequest;

use crate::eval_task::EvalTask;
use crate::fixture_index::{FIXTURE_HOME, FixtureIndex};
use crate::judge::Judge;
use crate::task_outcome::TaskOutcome;
use crate::task_set::TaskSet;
use crate::tpot_comparison::TpotComparison;

const TPOT_SAMPLES: usize = 8;
const TPOT_TOKENS: u32 = 128;
const TPOT_MINIMUM_TOKENS: u32 = 8;
const TPOT_INSTRUCTIONS: &str = "Ты — Домовой, помощник по месту на диске Mac. Отвечай кратко.";

#[derive(Debug)]
pub struct Evaluation<'a> {
    pub fixture: &'a FixtureIndex,
    pub tasks: &'a TaskSet,
    pub template: ChatTemplate,
    pub limits: AgentLimits,
}

impl Evaluation<'_> {
    pub fn answer_all(&self, backend: &mut dyn InferenceBackend) -> Vec<TaskOutcome> {
        self.tasks
            .tasks
            .iter()
            .map(|task| self.answer(&mut *backend, task))
            .collect()
    }

    fn answer(&self, backend: &mut dyn InferenceBackend, task: &EvalTask) -> TaskOutcome {
        let history = task.history();
        let context = AgentContext {
            index: &self.fixture.index,
            rules: &self.fixture.rules,
            denylist: &self.fixture.denylist,
            totals: self.fixture.totals,
            home: Path::new(FIXTURE_HOME),
            history: &history,
            focus: FocusContext::none(),
        };
        let mut tools = Vec::new();
        let result = Agent::new(backend, self.template, self.limits).ask(
            &context,
            &task.question,
            &CancelFlag::default(),
            &mut |step| tools.push(step.tool.name().to_owned()),
        );
        TaskOutcome {
            id: task.id.clone(),
            first_tool: tools.first().cloned(),
            tools,
            schema_valid: !matches!(result, Err(AgentError::InvalidResponse { .. })),
            answer: result.as_ref().ok().map(|answer| answer.text.clone()),
            error: result.err().map(|error| error.to_string()),
            score: None,
            judge_reason: None,
        }
    }

    pub fn judge_all(&self, backend: &mut dyn InferenceBackend, judge: &Judge, outcomes: &mut [TaskOutcome]) {
        for (task, outcome) in self.tasks.tasks.iter().zip(outcomes.iter_mut()) {
            if let Some(answer) = &outcome.answer
                && let Some((score, reason)) = judge.score(backend, task, answer)
            {
                outcome.score = Some(score);
                outcome.judge_reason = Some(reason);
            }
        }
    }

    pub fn compare_time_per_token(&self, backend: &mut dyn InferenceBackend) -> TpotComparison {
        let schema = ResponseSchema::answers_and_tool_calls();
        let system = system_prompt(TPOT_INSTRUCTIONS, &schema);
        let grammar = schema.grammar().as_str().to_owned();
        let mut with_grammar = Vec::new();
        let mut without_grammar = Vec::new();
        for task in self.tasks.tasks.iter().take(TPOT_SAMPLES) {
            let prompt = self
                .template
                .render(&system, &[ChatMessage::user(task.question.as_str())]);
            with_grammar.extend(time_per_token(backend, &prompt, Some(&grammar), self.limits));
            without_grammar.extend(time_per_token(backend, &prompt, None, self.limits));
        }
        TpotComparison {
            with_grammar_seconds: mean(&with_grammar),
            without_grammar_seconds: mean(&without_grammar),
        }
    }
}

fn time_per_token(
    backend: &mut dyn InferenceBackend,
    prompt: &str,
    grammar: Option<&str>,
    limits: AgentLimits,
) -> Option<f64> {
    let request = GenerateRequest {
        prompt: prompt.to_owned(),
        grammar: grammar.map(str::to_owned),
        sampling: limits.sampling,
        max_tokens: TPOT_TOKENS,
    };
    let stats = backend.generate(&request, &CancelFlag::default(), &mut |_| {}).ok()?;
    (stats.generated_tokens >= TPOT_MINIMUM_TOKENS)
        .then(|| stats.generation_duration.as_secs_f64() / f64::from(stats.generated_tokens))
        .filter(|seconds| *seconds > Duration::ZERO.as_secs_f64())
}

fn mean(values: &[f64]) -> f64 {
    assert!(!values.is_empty(), "at least one generation was long enough to time");
    values.iter().sum::<f64>() / values.len() as f64
}
