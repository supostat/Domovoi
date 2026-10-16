use domovoi_grammar::{CompiledSchema, ResponseSchema, SchemaProblem};
use serde_json::{Value, json};

fn problem(schema: &Value) -> SchemaProblem {
    CompiledSchema::compile(schema)
        .expect_err("the schema leaves the safe subset")
        .problem
}

fn object_with(property: &Value) -> Value {
    json!({
        "type": "object",
        "properties": {"field": property},
        "required": ["field"],
        "additionalProperties": false
    })
}

#[test]
fn response_grammars_match_their_golden_files() {
    assert_eq!(
        ResponseSchema::answers_and_tool_calls().grammar().as_str(),
        include_str!("golden/response.gbnf")
    );
    assert_eq!(
        ResponseSchema::tool_calls_only().grammar().as_str(),
        include_str!("golden/tool-calls.gbnf")
    );
}

#[test]
fn every_supported_construct_matches_its_golden_grammar() {
    let schema: Value =
        serde_json::from_str(include_str!("golden/constructs.schema.json")).expect("the golden schema parses");
    let compiled = CompiledSchema::compile(&schema).expect("the golden schema stays inside the safe subset");
    assert_eq!(compiled.grammar().as_str(), include_str!("golden/constructs.gbnf"));
}

#[test]
fn alternatives_pattern_properties_and_nested_references_are_refused() {
    assert_eq!(
        problem(&json!({
            "type": "object",
            "properties": {},
            "anyOf": [{"type": "boolean"}],
            "additionalProperties": false
        })),
        SchemaProblem::AlternativesBesideProperties
    );
    assert_eq!(
        problem(&json!({"type": "object", "patternProperties": {"^a$": {"type": "boolean"}}})),
        SchemaProblem::PatternProperties
    );
    assert_eq!(
        problem(&json!({
            "$defs": {"inner": {"type": "boolean"}, "outer": {"$ref": "#/$defs/inner"}},
            "$ref": "#/$defs/outer"
        })),
        SchemaProblem::NestedReference
    );
    assert_eq!(
        problem(&json!({"$ref": "https://example.com/schema.json"})),
        SchemaProblem::ExternalReference("https://example.com/schema.json".to_owned())
    );
    assert_eq!(
        problem(&json!({"$ref": "#/$defs/missing"})),
        SchemaProblem::UnknownReference("missing".to_owned())
    );
}

#[test]
fn open_objects_and_unbounded_values_are_refused() {
    assert_eq!(
        problem(&json!({"type": "object", "properties": {}})),
        SchemaProblem::AdditionalPropertiesAllowed
    );
    assert_eq!(
        problem(&json!({"type": "object", "properties": {}, "additionalProperties": true})),
        SchemaProblem::AdditionalPropertiesAllowed
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "string"}))),
        SchemaProblem::UnboundedString
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "string", "maxLength": 2000}))),
        SchemaProblem::StringTooLong(2000)
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "array", "items": {"type": "boolean"}}))),
        SchemaProblem::UnboundedArray
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "integer", "minimum": 0}))),
        SchemaProblem::HalfOpenRange
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "number", "minimum": 0.5, "maximum": 1}))),
        SchemaProblem::FractionalBound
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "number", "minimum": -1, "maximum": 1}))),
        SchemaProblem::NegativeNumberBound
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "integer", "minimum": 5, "maximum": 1}))),
        SchemaProblem::InvertedBounds
    );
}

#[test]
fn patterns_must_be_anchored_and_bounded() {
    assert_eq!(
        problem(&object_with(&json!({"type": "string", "pattern": "[a-z]{0,3}"}))),
        SchemaProblem::UnanchoredPattern
    );
    assert!(matches!(
        problem(&object_with(&json!({"type": "string", "pattern": "^[a-z]+$"}))),
        SchemaProblem::UnsupportedPattern(_)
    ));
    assert_eq!(
        problem(&object_with(
            &json!({"type": "string", "pattern": "^a$", "maxLength": 3})
        )),
        SchemaProblem::PatternWithLength
    );
}

#[test]
fn unknown_keywords_types_and_enums_are_refused() {
    assert_eq!(
        problem(&object_with(
            &json!({"type": "string", "maxLength": 9, "format": "date"})
        )),
        SchemaProblem::UnsupportedKeyword("format".to_owned())
    );
    assert_eq!(
        problem(&object_with(&json!({"type": "null"}))),
        SchemaProblem::UnknownType("null".to_owned())
    );
    assert_eq!(
        problem(&object_with(&json!({"maxLength": 9}))),
        SchemaProblem::MissingType
    );
    assert_eq!(problem(&object_with(&json!({"enum": []}))), SchemaProblem::EmptyEnum);
    assert_eq!(
        problem(&object_with(&json!({"enum": [{"nested": true}]}))),
        SchemaProblem::UnsupportedEnumValue
    );
    assert_eq!(
        problem(&json!({
            "type": "object",
            "properties": {},
            "required": ["ghost"],
            "additionalProperties": false
        })),
        SchemaProblem::UndeclaredRequired("ghost".to_owned())
    );
}
