use domovoi_agent::{ClaudeCode, ClaudeCodeError};
use domovoi_inference::CancelFlag;

use crate::eval_task::EvalTask;
use crate::fixture_index::FixtureIndex;
use crate::judge::{RUBRIC, judged_message, verdict_document, verdict_of, verdict_schema};
use crate::task_outcome::TaskOutcome;
use crate::task_set::TaskSet;

#[derive(Debug)]
pub struct ClaudeEvaluation<'a> {
    pub fixture: &'a FixtureIndex,
    pub tasks: &'a TaskSet,
    pub claude: &'a ClaudeCode,
}

impl ClaudeEvaluation<'_> {
    pub fn answer_all(&self) -> Vec<TaskOutcome> {
        self.tasks.tasks.iter().map(|task| self.answer(task)).collect()
    }

    pub fn judge_all(&self, outcomes: &mut [TaskOutcome]) {
        let schema = verdict_schema();
        let document = verdict_document();
        for (task, outcome) in self.tasks.tasks.iter().zip(outcomes.iter_mut()) {
            if let Some(answer) = &outcome.answer
                && let Ok(verdict) =
                    self.claude
                        .structured(RUBRIC, &judged_message(task, answer), &document, &CancelFlag::default())
                && let Some((score, reason)) = verdict_of(&schema, &verdict)
            {
                outcome.score = Some(score);
                outcome.judge_reason = Some(reason);
            }
        }
    }

    fn answer(&self, task: &EvalTask) -> TaskOutcome {
        let history = task.history();
        let mut tools = Vec::new();
        let result = self.claude.ask(
            &self.fixture.context(&history),
            &task.question,
            None,
            &CancelFlag::default(),
            &mut |step| tools.push(step.tool.name().to_owned()),
        );
        TaskOutcome {
            id: task.id.clone(),
            first_tool: tools.first().cloned(),
            tools,
            schema_valid: !matches!(result, Err(ClaudeCodeError::Output(_))),
            answer: result.as_ref().ok().map(|answer| answer.answer.text.clone()),
            error: result.err().map(|error| error.to_string()),
            score: None,
            judge_reason: None,
        }
    }
}
