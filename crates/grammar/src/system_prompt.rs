use std::fmt::Write;

use serde_json::json;

use crate::response_schema::ResponseSchema;
use crate::tool::Tool;

pub fn system_prompt(instructions: &str, schema: &ResponseSchema) -> String {
    let mut prompt = format!("{}\n\n# Инструменты\n", instructions.trim());
    for tool in Tool::ALL {
        writeln!(prompt, "- {}: {}", tool.name(), tool.description()).expect("writing to a string succeeds");
    }
    writeln!(
        prompt,
        "\n# Формат ответа\nКаждый ответ — один JSON-объект по схеме:\n{}",
        schema.document()
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
                    "path": "/Users/igor/Library/Application Support/com.tinyapp.helper",
                    "guess": "Похоже на данные удалённого приложения TinyApp",
                    "confidence": 0.6
                }]
            })
        )
        .expect("writing to a string succeeds");
    }
    prompt.trim_end().to_owned()
}
