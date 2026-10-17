use serde_json::Value;

pub(crate) fn untrusted_json(value: &Value) -> String {
    value.to_string().replace('<', "\\u003c").replace('>', "\\u003e")
}

pub(crate) fn neutral_question(question: &str) -> String {
    question.replace('<', "‹").replace('>', "›")
}
