use std::fmt::Write;

pub(crate) fn literal(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control.is_control() => {
                write!(escaped, "\\x{:02X}", u32::from(control)).expect("writing to a string succeeds");
            }
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}

pub(crate) fn class_character(character: char) -> String {
    match character {
        ']' | '\\' | '^' | '-' | '[' => format!("\\{character}"),
        other => other.to_string(),
    }
}

pub(crate) fn repeated(expression: &str, minimum: u32, maximum: u32) -> String {
    match (minimum, maximum) {
        (0, 1) => format!("{expression}?"),
        (1, 1) => expression.to_owned(),
        (minimum, maximum) if minimum == maximum => format!("{expression}{{{minimum}}}"),
        (minimum, maximum) => format!("{expression}{{{minimum},{maximum}}}"),
    }
}

pub(crate) fn rule_name(path: &str) -> String {
    let mut name = String::with_capacity(path.len());
    let mut previous_dash = true;
    for character in path.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            name.push('-');
            previous_dash = true;
        }
    }
    while name.ends_with('-') {
        name.pop();
    }
    name
}
