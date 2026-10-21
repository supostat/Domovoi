use domovoi_agent::{PastTurn, ToolStep};
use domovoi_app::{AnswerTarget, ChatEntry, ChatModel};
use domovoi_grammar::Tool;
use serde_json::{Value, json};

const KEPT_TURNS: usize = 6;
const SUPPORT: &str = "~/Library/Application Support";
const OLD_APP: &str = "~/Library/Application Support/OldApp";
const TARGET: &str = "/Volumes/Data/cache";

fn step(tool: Tool, arguments: &Value) -> ToolStep {
    ToolStep {
        tool,
        arguments: arguments.as_object().cloned().expect("arguments are an object"),
        result: Value::Null,
        nodes: Vec::new(),
    }
}

fn answer(text: &str) -> ChatEntry {
    ChatEntry::Answer {
        text: text.to_owned(),
        items: Vec::new(),
        targets: Vec::new(),
    }
}

#[test]
fn a_finished_turn_carries_its_question_answer_and_every_mentioned_path() {
    let mut chat = ChatModel::default();
    chat.ask("Что в Application Support?");
    chat.record_step(&step(Tool::InspectNode, &json!({"path": SUPPORT})));
    chat.record_step(&step(Tool::QueryIndex, &json!({"path_prefix": SUPPORT, "limit": 5})));
    chat.entries.push(ChatEntry::Answer {
        text: "Там данные приложений.".to_owned(),
        items: Vec::new(),
        targets: vec![AnswerTarget {
            path: TARGET.into(),
            rule_id: "cache.app.generic".to_owned(),
        }],
    });
    chat.entries.push(ChatEntry::Hypothesis {
        path: OLD_APP.to_owned(),
        text: "Похоже на данные удалённого приложения".to_owned(),
    });
    assert_eq!(
        chat.past_turns(),
        vec![PastTurn {
            question: "Что в Application Support?".to_owned(),
            answer: "Там данные приложений.".to_owned(),
            paths: vec![SUPPORT.to_owned(), TARGET.to_owned(), OLD_APP.to_owned()],
        }]
    );
}

#[test]
fn a_failure_and_an_unanswered_question_make_no_turn() {
    let mut chat = ChatModel::default();
    chat.ask("Первый вопрос");
    chat.record_problem("Модель не смогла ответить.");
    chat.ask("Второй вопрос");
    chat.entries.push(answer("Второй ответ"));
    chat.ask("Третий вопрос, ещё без ответа");
    let turns = chat.past_turns();
    assert_eq!(turns.len(), 1, "{turns:?}");
    assert_eq!(turns[0].question, "Второй вопрос");
    assert!(turns[0].paths.is_empty());
}

#[test]
fn only_the_latest_turns_are_kept() {
    let mut chat = ChatModel::default();
    for number in 1..=KEPT_TURNS + 2 {
        chat.ask(&format!("Вопрос {number}"));
        chat.entries.push(answer(&format!("Ответ {number}")));
    }
    let questions: Vec<String> = chat.past_turns().into_iter().map(|turn| turn.question).collect();
    let expected: Vec<String> = (3..=KEPT_TURNS + 2).map(|number| format!("Вопрос {number}")).collect();
    assert_eq!(questions, expected);
}
