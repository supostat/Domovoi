use std::collections::BTreeSet;
use std::env;
use std::ffi::CString;
use std::fs::{self, File};
use std::path::Path;

use domovoi_agent::AgentLimits;
use domovoi_downloader::Catalog;
use domovoi_eval::{EvalMetrics, EvalTask, Evaluation, FIXTURE_HOME, FixtureIndex, Judge, TaskSet};
use domovoi_grammar::{ChatTemplate, PromptFamily, Tool};
use domovoi_inference::{CancelFlag, DeviceProfile, FakeBackend, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::ModelClass;
use serde_json::json;

const MINIMUM_TASKS: usize = 70;
const MINIMUM_FOLLOW_UPS: usize = 6;
const TINY_MODEL: &str = "../../assets/test/tiny-random.gguf";
const FAKE_CONTEXT: u32 = 2048;
const FOLLOW_UP_CONTEXT: u32 = 65_536;
const FAKE_TEMPLATE: ChatTemplate = ChatTemplate {
    family: PromptFamily::ChatMl,
    empty_think_before_answer: true,
    empty_think_in_tool_steps: false,
};
const CLASS_VARIABLE: &str = "DOMOVOI_EVAL_CLASS";
const METALLIBS: [&str; 2] = ["default.metallib", "ggml-tensor.metallib"];
const REPORT_DIRECTORY: &str = "../../target/eval";
const CLAIMED_RULES: [&str; 12] = [
    "xcode.derived-data",
    "xcode.archives",
    "xcode.dyld-cache",
    "xcode.caches",
    "docker.vm",
    "ios.backups",
    "cache.homebrew",
    "cache.npm",
    "cache.cargo-registry",
    "downloads.installers",
    "cargo.target",
    "dev.deps",
];

fn is_russian(text: &str) -> bool {
    text.chars()
        .any(|letter| ('а'..='я').contains(&letter.to_lowercase().next().unwrap_or(letter)))
}

fn follow_ups() -> Vec<EvalTask> {
    TaskSet::bundled()
        .tasks
        .into_iter()
        .filter(|task| !task.before.is_empty())
        .collect()
}

fn loaded_fake(replies: Vec<String>) -> FakeBackend {
    let mut backend = FakeBackend::replying(replies);
    let weights = File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(TINY_MODEL)).expect("the weights open");
    backend
        .load(
            weights.into(),
            ModelClass::Compact,
            FAKE_CONTEXT,
            &CancelFlag::default(),
        )
        .expect("the fake backend loads");
    backend
}

#[test]
fn the_task_set_holds_seventy_russian_tasks_across_every_tool() {
    let tasks = TaskSet::bundled();
    assert!(tasks.tasks.len() >= MINIMUM_TASKS, "{} tasks", tasks.tasks.len());
    for tool in Tool::ALL {
        assert!(
            tasks.tasks.iter().any(|task| task.expected_tool == Some(tool)),
            "no task expects {}",
            tool.name()
        );
    }
    assert!(tasks.tasks.iter().any(|task| !task.needs_tool));
    for task in &tasks.tasks {
        assert!(is_russian(&task.question), "{} is not asked in Russian", task.id);
        assert!(!task.reference.trim().is_empty(), "{} has no reference", task.id);
    }
}

#[test]
fn six_follow_up_tasks_carry_earlier_turns_about_the_fixture() {
    let fixture = FixtureIndex::build();
    let follow_ups = follow_ups();
    assert!(
        follow_ups.len() >= MINIMUM_FOLLOW_UPS,
        "{} follow-ups",
        follow_ups.len()
    );
    for task in &follow_ups {
        for turn in &task.before {
            assert!(
                is_russian(&turn.question),
                "{}: an earlier question is not Russian",
                task.id
            );
            assert!(
                !turn.answer.trim().is_empty(),
                "{}: an earlier turn has no answer",
                task.id
            );
            for path in &turn.paths {
                assert!(
                    fixture.index.find(Path::new(path)).is_some(),
                    "{}: {path} is not in the fixture",
                    task.id
                );
            }
        }
    }
}

#[test]
fn follow_up_tasks_reach_the_agent_with_their_earlier_turns() {
    let fixture = FixtureIndex::build();
    let tasks = TaskSet { tasks: follow_ups() };
    let final_answer = json!({"kind": "final", "answer": "Ок.", "hypotheses": []});
    let mut backend = loaded_fake(tasks.tasks.iter().map(|_| format!("token {final_answer}")).collect());
    let evaluation = Evaluation {
        fixture: &fixture,
        tasks: &tasks,
        template: FAKE_TEMPLATE,
        limits: AgentLimits::for_context_length(FOLLOW_UP_CONTEXT),
    };
    let outcomes = evaluation.answer_all(&mut backend);
    for ((task, outcome), request) in tasks.tasks.iter().zip(&outcomes).zip(backend.requests()) {
        assert_eq!(
            outcome.answer.as_deref(),
            Some("Ок."),
            "{}: {:?}",
            task.id,
            outcome.error
        );
        let prompt = &request.prompt;
        let current = prompt
            .rfind(&task.question)
            .unwrap_or_else(|| panic!("{}: the question is in the prompt", task.id));
        for turn in &task.before {
            let earlier = prompt
                .find(&turn.question)
                .unwrap_or_else(|| panic!("{}: an earlier question is in the prompt", task.id));
            assert!(earlier < current, "{}: {prompt}", task.id);
            assert!(prompt.contains(&turn.answer), "{}: {prompt}", task.id);
        }
        let last = task.before.last().expect("a follow-up has an earlier turn");
        if !last.paths.is_empty() {
            assert!(
                prompt.contains(&format!(r#""last_paths":{}"#, json!(last.paths))),
                "{}: {prompt}",
                task.id
            );
        }
    }
}

#[test]
fn the_fixture_index_is_claimed_by_the_rules_the_tasks_rely_on() {
    let fixture = FixtureIndex::build();
    let claimed: BTreeSet<&str> = fixture
        .rules
        .claims(&fixture.index)
        .iter()
        .map(|claim| fixture.rules.rule(claim.rule).id)
        .collect();
    for rule in CLAIMED_RULES {
        assert!(claimed.contains(rule), "{rule} claims nothing in {claimed:?}");
    }
    assert!(fixture.totals.safe > 0 && fixture.totals.check_yourself > 0);
    for task in TaskSet::bundled().tasks {
        let Some(start) = task.question.find(FIXTURE_HOME) else {
            continue;
        };
        let words: Vec<&str> = task.question[start..].split(' ').collect();
        let resolves = (1..=words.len()).rev().any(|count| {
            let candidate = words[..count].join(" ");
            let candidate = candidate.trim_end_matches(['?', '.', ',']);
            fixture.index.find(Path::new(candidate)).is_some()
        });
        assert!(
            resolves || task.id == "inspect-missing",
            "{}: the path is not in the fixture",
            task.id
        );
    }
}

#[test]
#[ignore = "runs the primary catalog model of a class on the nightly self-hosted machine"]
fn the_primary_model_of_a_class_meets_the_eval_thresholds() {
    let class_name =
        env::var(CLASS_VARIABLE).unwrap_or_else(|_| panic!("{CLASS_VARIABLE} names the class to evaluate"));
    let class = CString::new(class_name.as_str())
        .ok()
        .and_then(|name| ModelClass::from_name(&name))
        .unwrap_or_else(|| panic!("{class_name} is not a model class"));
    let entry = Catalog::bundled()
        .primary(class)
        .expect("every class has a primary entry")
        .clone();
    let model_variable = format!("DOMOVOI_NIGHTLY_{}_MODEL", class_name.to_uppercase());
    let model =
        env::var_os(&model_variable).unwrap_or_else(|| panic!("{model_variable} names the GGUF of {}", entry.id));
    let binary = env::current_exe().expect("the test binary is known");
    let beside = binary.parent().expect("the test binary lies in a directory");
    for metallib in METALLIBS {
        fs::copy(
            Path::new(domovoi_llama_sys::METALLIB_DIRECTORY).join(metallib),
            beside.join(metallib),
        )
        .expect("a metallib is copied");
    }
    let mut backend = LlamaBackend::new(DeviceProfile::Gpu);
    backend
        .load(
            File::open(model).expect("the model opens").into(),
            class,
            entry.context_length,
            &CancelFlag::default(),
        )
        .expect("the model loads on Metal");

    let fixture = FixtureIndex::build();
    let tasks = TaskSet::bundled();
    let evaluation = Evaluation {
        fixture: &fixture,
        tasks: &tasks,
        template: entry.template,
        limits: AgentLimits::for_context_length(entry.context_length),
    };
    let mut outcomes = evaluation.answer_all(&mut backend);
    evaluation.judge_all(&mut backend, &Judge::new(entry.template), &mut outcomes);
    let tpot = evaluation.compare_time_per_token(&mut backend);
    let metrics = EvalMetrics::measure(&tasks, &outcomes, tpot);

    let reports = Path::new(env!("CARGO_MANIFEST_DIR")).join(REPORT_DIRECTORY);
    fs::create_dir_all(&reports).expect("the report directory is created");
    let report =
        json!({"class": class_name, "entry": entry.id, "metrics": metrics, "tpot": tpot, "outcomes": outcomes});
    fs::write(
        reports.join(format!("{class_name}.json")),
        serde_json::to_string_pretty(&report).expect("the report serialises"),
    )
    .expect("the report is written");
    eprintln!("{} on {class_name}: {metrics:?}", entry.id);
    let shortfalls = metrics.shortfalls();
    assert!(
        shortfalls.is_empty(),
        "{} misses the thresholds: {shortfalls:?}",
        entry.id
    );
}
