use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domovoi_grammar::{Tool, ToolCall};
use domovoi_index::{Category, NodeId, NodeKind, Query};
use serde_json::{Map, Value, json};

use crate::agent_context::AgentContext;
use crate::level_name::{level_name, level_named};
use crate::tool_outcome::ToolOutcome;

const DEFAULT_LIMIT: usize = 10;
const LARGEST_CHILDREN: usize = 5;
const LARGEST_RULES: usize = 10;
const SECONDS_PER_DAY: u64 = 86_400;

pub(crate) struct ToolRunner<'a> {
    context: &'a AgentContext<'a>,
    now: SystemTime,
}

impl<'a> ToolRunner<'a> {
    pub(crate) fn new(context: &'a AgentContext<'a>) -> Self {
        Self {
            context,
            now: SystemTime::now(),
        }
    }

    pub(crate) fn run(&self, call: &ToolCall) -> ToolOutcome {
        match call.tool {
            Tool::QueryIndex => self.query_index(&call.arguments),
            Tool::InspectNode => self.inspect_node(required_text(&call.arguments, "path")),
            Tool::RuleExplain => self.rule_explain(required_text(&call.arguments, "id")),
            Tool::Summarize => ToolOutcome {
                result: self.summary(),
                nodes: Vec::new(),
            },
        }
    }

    pub(crate) fn overview(&self) -> Value {
        let index = self.context.index;
        let totals = self.context.totals;
        json!({
            "used_bytes": index.node(index.root()).allocated,
            "reclaimable": {
                "safe": totals.safe,
                "recoverable": totals.recoverable,
                "check_yourself": totals.check_yourself,
                "total": totals.total()
            },
            "categories": self.category_totals()
        })
    }

    fn summary(&self) -> Value {
        let mut summary = self.overview();
        summary["rules"] = self.rule_totals();
        summary
    }

    fn query_index(&self, arguments: &Map<String, Value>) -> ToolOutcome {
        let text = |name: &str| arguments.get(name).and_then(Value::as_str);
        let number = |name: &str| arguments.get(name).and_then(Value::as_u64);
        let query = Query {
            category: text("category")
                .and_then(|name| Category::ALL.into_iter().find(|category| category.id() == name)),
            level: text("level").and_then(level_named),
            min_allocated: number("min_size_bytes").unwrap_or(0),
            inactive_since: number("inactive_days").map(|days| {
                self.now
                    .checked_sub(Duration::from_secs(days * SECONDS_PER_DAY))
                    .unwrap_or(UNIX_EPOCH)
            }),
            path_prefix: text("path_prefix").map(PathBuf::from),
        };
        let limit = number("limit").map_or(DEFAULT_LIMIT, |limit| limit as usize);
        let matches = self.context.index.query(&query, self.context.rules);
        let nodes: Vec<NodeId> = matches.iter().copied().take(limit).collect();
        let items: Vec<Value> = nodes.iter().map(|node| self.describe(*node)).collect();
        ToolOutcome {
            result: json!({"total": matches.len(), "items": items}),
            nodes,
        }
    }

    fn inspect_node(&self, path: &str) -> ToolOutcome {
        let index = self.context.index;
        let Some(node) = index.find(Path::new(path)) else {
            return ToolOutcome {
                result: json!({"error": "узел не найден", "path": path}),
                nodes: Vec::new(),
            };
        };
        let mut children: Vec<NodeId> = index.children(node).collect();
        children.sort_by_key(|child| std::cmp::Reverse(index.node(*child).allocated));
        children.truncate(LARGEST_CHILDREN);
        let mut result = self.describe(node);
        result["largest_children"] = children.iter().map(|child| self.describe(*child)).collect();
        let mut nodes = vec![node];
        nodes.extend(children);
        ToolOutcome { result, nodes }
    }

    fn rule_explain(&self, id: &str) -> ToolOutcome {
        let result = self
            .context
            .rules
            .rules()
            .iter()
            .find(|rule| rule.id == id)
            .map_or_else(
                || json!({"error": "правило не найдено", "id": id}),
                |rule| {
                    json!({
                        "id": rule.id,
                        "level": level_name(rule.level),
                        "explanation": rule.explanation,
                        "owner_bundle": rule.owner_bundle,
                        "vendor_command": rule.vendor_command
                    })
                },
            );
        ToolOutcome {
            result,
            nodes: Vec::new(),
        }
    }

    fn describe(&self, node: NodeId) -> Value {
        let index = self.context.index;
        let record = index.node(node);
        let rule = self
            .context
            .rules
            .claim_of(index, node)
            .map(|claim| self.context.rules.rule(claim.rule));
        let kind = if record.kind == NodeKind::Directory {
            "directory"
        } else {
            "file"
        };
        json!({
            "path": index.path(node).to_string_lossy(),
            "kind": kind,
            "category": record.category.id(),
            "bytes": record.allocated,
            "files": record.files,
            "days_since_activity": self.days_since_activity(node),
            "level": rule.map(|rule| level_name(rule.level)),
            "rule": rule.map(|rule| rule.id)
        })
    }

    fn days_since_activity(&self, node: NodeId) -> Option<u64> {
        self.context
            .index
            .last_activity(node)
            .system_time()
            .and_then(|activity| self.now.duration_since(activity).ok())
            .map(|elapsed| elapsed.as_secs() / SECONDS_PER_DAY)
    }

    fn category_totals(&self) -> Value {
        let index = self.context.index;
        let mut totals: HashMap<Category, u64> = HashMap::new();
        let mut pending = vec![index.root()];
        while let Some(node) = pending.pop() {
            let before = pending.len();
            pending.extend(index.children(node));
            if pending.len() == before {
                let record = index.node(node);
                *totals.entry(record.category).or_default() += record.allocated;
            }
        }
        let mut sorted: Vec<(Category, u64)> = totals.into_iter().filter(|(_, bytes)| *bytes > 0).collect();
        sorted.sort_by_key(|(category, bytes)| (std::cmp::Reverse(*bytes), category.id()));
        sorted
            .into_iter()
            .map(|(category, bytes)| json!({"category": category.id(), "bytes": bytes}))
            .collect()
    }

    fn rule_totals(&self) -> Value {
        let index = self.context.index;
        let rules = self.context.rules;
        let mut totals: HashMap<usize, (u64, usize)> = HashMap::new();
        for claim in rules.claims(index) {
            let entry = totals.entry(claim.rule).or_default();
            entry.0 += index.node(claim.node).allocated;
            entry.1 += 1;
        }
        let mut sorted: Vec<(usize, (u64, usize))> = totals.into_iter().collect();
        sorted.sort_by_key(|(rule, (bytes, _))| (std::cmp::Reverse(*bytes), *rule));
        sorted
            .into_iter()
            .take(LARGEST_RULES)
            .map(|(rule, (bytes, items))| {
                let rule = rules.rule(rule);
                json!({
                    "rule": rule.id,
                    "level": level_name(rule.level),
                    "bytes": bytes,
                    "items": items,
                    "info_only": rule.is_info_only()
                })
            })
            .collect()
    }
}

fn required_text<'a>(arguments: &'a Map<String, Value>, name: &str) -> &'a str {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .expect("the response schema requires this text argument")
}
