use domovoi_grammar::{
    FinalAnswer, ModelHypothesis, ModelResponse, ResponseError, ResponseSchema, Tool, system_prompt,
};
use serde_json::json;

fn rejects(schema: &ResponseSchema, text: &str) -> ResponseError {
    schema.parse(text).expect_err("the response deviates from the schema")
}

#[test]
fn tool_calls_and_final_answers_parse_into_typed_responses() {
    let schema = ResponseSchema::answers_and_tool_calls();
    let call = schema
        .parse(r#"{"kind":"tool_call","name":"query_index","arguments":{"category":"cache","limit":10}}"#)
        .expect("a valid tool call parses");
    let ModelResponse::ToolCall(call) = call else {
        panic!("expected a tool call");
    };
    assert_eq!(call.tool, Tool::QueryIndex);
    assert_eq!(
        serde_json::Value::Object(call.arguments),
        json!({"category": "cache", "limit": 10})
    );
    let answer = schema
        .parse(
            r#" {"kind": "final", "answer": "Кэши Xcode: 38,2 ГБ", "hypotheses": [{"path": "/Users/igor/x", "guess": "Похоже на кэш", "confidence": 0.75}]} "#,
        )
        .expect("a valid answer parses");
    assert_eq!(
        answer,
        ModelResponse::Final(FinalAnswer {
            answer: "Кэши Xcode: 38,2 ГБ".to_owned(),
            hypotheses: vec![ModelHypothesis {
                path: "/Users/igor/x".to_owned(),
                guess: "Похоже на кэш".to_owned(),
                confidence: 0.75,
            }],
        })
    );
}

#[test]
fn malformed_or_repeated_json_is_rejected() {
    let schema = ResponseSchema::answers_and_tool_calls();
    assert!(matches!(
        rejects(&schema, r#"{"kind":"tool_call""#),
        ResponseError::Json(_)
    ));
    assert!(matches!(
        rejects(
            &schema,
            r#"{"kind":"tool_call","name":"summarize","arguments":{}} trailing"#
        ),
        ResponseError::Json(_)
    ));
    assert_eq!(
        rejects(
            &schema,
            r#"{"kind":"tool_call","name":"summarize","name":"summarize","arguments":{}}"#
        ),
        ResponseError::DuplicateKey("name".to_owned())
    );
}

#[test]
fn every_schema_deviation_is_rejected() {
    let schema = ResponseSchema::answers_and_tool_calls();
    for deviation in [
        r#"{"kind":"tool_call","name":"summarize","arguments":{},"extra":1}"#,
        r#"{"kind":"tool_call","name":"summarize"}"#,
        r#"{"kind":"call","name":"summarize","arguments":{}}"#,
        r#"{"kind":"tool_call","name":"delete_everything","arguments":{}}"#,
        r#"{"kind":"tool_call","name":"inspect_node","arguments":{"id":"xcode.derived-data"}}"#,
        r#"{"kind":"tool_call","name":"query_index","arguments":{"limit":51}}"#,
        r#"{"kind":"tool_call","name":"query_index","arguments":{"limit":5.0}}"#,
        r#"{"kind":"tool_call","name":"query_index","arguments":{"category":"music"}}"#,
        r#"{"kind":"tool_call","name":"rule_explain","arguments":{"id":"Xcode"}}"#,
        r#"{"kind":"final","answer":"","hypotheses":[]}"#,
        r#"{"kind":"final","answer":"x","hypotheses":[{"path":"/a","guess":"b","confidence":1.5}]}"#,
        r#"{"kind":"final","answer":"x","hypotheses":[{"path":"/a","guess":"b"}]}"#,
        r#"{"kind":"final","answer":"x"}"#,
        r#"[{"kind":"final","answer":"x","hypotheses":[]}]"#,
    ] {
        assert!(
            matches!(rejects(&schema, deviation), ResponseError::Schema(_)),
            "{deviation}"
        );
    }
    let too_many: Vec<_> = (0..6)
        .map(|position| json!({"path": format!("/{position}"), "guess": "g", "confidence": 0.5}))
        .collect();
    let crowded = json!({"kind": "final", "answer": "x", "hypotheses": too_many}).to_string();
    assert!(matches!(rejects(&schema, &crowded), ResponseError::Schema(_)));
    let long_answer = json!({"kind": "final", "answer": "ж".repeat(1501), "hypotheses": []}).to_string();
    assert!(matches!(rejects(&schema, &long_answer), ResponseError::Schema(_)));
}

#[test]
fn the_first_step_grammar_admits_only_tool_calls() {
    let schema = ResponseSchema::tool_calls_only();
    assert!(
        schema
            .parse(r#"{"kind":"tool_call","name":"summarize","arguments":{}}"#)
            .is_ok()
    );
    assert!(matches!(
        rejects(&schema, r#"{"kind":"final","answer":"x","hypotheses":[]}"#),
        ResponseError::Schema(_)
    ));
    let prompt = system_prompt("Ты — Домовой.", &schema);
    assert!(
        !prompt.contains(&schema.document().to_string()),
        "the grammar holds the form, so the prompt carries no schema: {prompt}"
    );
    assert!(prompt.contains("Пример вызова инструмента"));
    assert!(!prompt.contains("Пример ответа"));
    for tool in Tool::ALL {
        assert!(prompt.contains(&format!("- {}(", tool.name())), "{}", tool.name());
    }
    assert!(prompt.contains("level?: safe|recoverable|check_yourself"), "{prompt}");
    assert!(prompt.contains("path_prefix?: строка"), "{prompt}");
    assert!(prompt.contains("inspect_node(path: строка)"), "{prompt}");
}

#[test]
fn the_last_step_grammar_admits_only_a_final_answer() {
    let schema = ResponseSchema::answers_only();
    assert!(
        schema
            .parse(r#"{"kind":"final","answer":"Можно освободить 3,2 ГБ","hypotheses":[]}"#)
            .is_ok()
    );
    assert!(matches!(
        rejects(&schema, r#"{"kind":"tool_call","name":"summarize","arguments":{}}"#),
        ResponseError::Schema(_)
    ));
    assert!(!schema.grammar().as_str().contains("tool_call"));
}

#[test]
fn the_system_prompt_names_no_user_and_writes_paths_from_home() {
    let prompt = system_prompt("Ты — Домовой.", &ResponseSchema::answers_and_tool_calls());
    assert!(prompt.contains("Пример ответа"));
    assert!(!prompt.contains("/Users/"), "{prompt}");
    assert!(prompt.contains("\"path\":\"~/"), "{prompt}");
    assert!(
        !prompt.contains("Application Support"),
        "the example does not steer guesses to one folder: {prompt}"
    );
}
