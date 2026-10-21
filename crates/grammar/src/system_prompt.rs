use std::fmt::Write;

use serde_json::{Value, json};

use crate::response_schema::ResponseSchema;
use crate::tool::Tool;

pub fn system_prompt(instructions: &str, schema: &ResponseSchema) -> String {
    let mut prompt = format!("{}\n\n# Инструменты\n", instructions.trim());
    for tool in Tool::ALL {
        writeln!(prompt, "- {}: {}", signature(tool), tool.description()).expect("writing to a string succeeds");
    }
    writeln!(
        prompt,
        "\n# Формат ответа\nКаждый ответ — один JSON-объект, как в примерах."
    )
    .expect("writing to a string succeeds");
    writeln!(
        prompt,
        "\nПример вызова инструмента:\n{}",
        json!({"kind": "tool_call", "name": Tool::Summarize.name(), "arguments": {}})
    )
    .expect("writing to a string succeeds");
    if schema.allows_final() {
        writeln!(
            prompt,
            "\nПример ответа:\n{}",
            json!({
                "kind": "final",
                "answer": "Больше всего места занимают сборки Xcode: 38,2 ГБ в DerivedData.",
                "hypotheses": [{
                    "path": "~/Downloads/tinyapp-backup",
                    "guess": "Похоже на резервную копию удалённого приложения TinyApp",
                    "confidence": 0.6
                }]
            })
        )
        .expect("writing to a string succeeds");
    }
    prompt.trim_end().to_owned()
}

fn signature(tool: Tool) -> String {
    let schema = tool.arguments_schema();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let arguments: Vec<String> = schema["properties"]
        .as_object()
        .expect("a tool schema lists its properties")
        .iter()
        .map(|(name, property)| {
            let optional = if required.contains(&name.as_str()) { "" } else { "?" };
            format!("{name}{optional}: {}", kind(property))
        })
        .collect();
    format!("{}({})", tool.name(), arguments.join(", "))
}

fn kind(property: &Value) -> String {
    if let Some(values) = property["enum"].as_array() {
        return values.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("|");
    }
    match property["type"].as_str() {
        Some("integer") => "число".to_owned(),
        _ => "строка".to_owned(),
    }
}
