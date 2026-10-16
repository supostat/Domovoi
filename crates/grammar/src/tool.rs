use serde_json::{Value, json};

const CATEGORIES: [&str; 9] = [
    "apps", "dev", "media", "docs", "cache", "system", "hidden", "model", "home",
];
const SAFETY_LEVELS: [&str; 3] = ["safe", "recoverable", "check_yourself"];
const LARGEST_SIZE_BYTES: i64 = 999_999_999_999_999;
const LONGEST_INACTIVITY_DAYS: i64 = 36_500;
const LARGEST_LIMIT: i64 = 50;
const LONGEST_PATH: u64 = 1024;
const RULE_ID_PATTERN: &str = "^[a-z][a-z0-9.-]{0,63}$";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    QueryIndex,
    InspectNode,
    RuleExplain,
    Summarize,
}

impl Tool {
    pub const ALL: [Self; 4] = [Self::QueryIndex, Self::InspectNode, Self::RuleExplain, Self::Summarize];

    pub fn name(self) -> &'static str {
        match self {
            Self::QueryIndex => "query_index",
            Self::InspectNode => "inspect_node",
            Self::RuleExplain => "rule_explain",
            Self::Summarize => "summarize",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::QueryIndex => {
                "найти узлы индекса диска по категории, уровню безопасности, размеру, давности и префиксу пути"
            }
            Self::InspectNode => "показать сведения об одном файле или папке по полному пути",
            Self::RuleExplain => "объяснить правило рекомендаций по его идентификатору",
            Self::Summarize => "дать сводку по диску: занято, свободно и итоги по уровням",
        }
    }

    pub fn arguments_schema(self) -> Value {
        match self {
            Self::QueryIndex => json!({
                "type": "object",
                "properties": {
                    "category": {"enum": CATEGORIES},
                    "level": {"enum": SAFETY_LEVELS},
                    "min_size_bytes": {"type": "integer", "minimum": 0, "maximum": LARGEST_SIZE_BYTES},
                    "inactive_days": {"type": "integer", "minimum": 0, "maximum": LONGEST_INACTIVITY_DAYS},
                    "path_prefix": {"type": "string", "minLength": 1, "maxLength": LONGEST_PATH},
                    "limit": {"type": "integer", "minimum": 1, "maximum": LARGEST_LIMIT}
                },
                "additionalProperties": false
            }),
            Self::InspectNode => json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "minLength": 1, "maxLength": LONGEST_PATH}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            Self::RuleExplain => json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "pattern": RULE_ID_PATTERN}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            Self::Summarize => json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }
}
