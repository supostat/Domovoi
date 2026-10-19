use std::ffi::OsStr;
use std::path::{Component, Path};

use crate::name_match::{CaseRule, Segment};

const HOME_PREFIX: &str = "~/";
const ANY_DEPTH_PREFIX: &str = "**/";
const HOME_COMPONENTS: [&str; 2] = ["Users", "*"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PathPattern {
    Anchored(Vec<Segment>),
    AnyDepth {
        name: Segment,
        sibling_marker: Option<String>,
    },
}

impl PathPattern {
    pub(crate) fn parse(pattern: &str) -> Self {
        if let Some(rest) = pattern.strip_prefix(ANY_DEPTH_PREFIX) {
            let (name, marker) = match rest.split_once('|') {
                Some((name, marker)) => (name, Some(marker.to_owned())),
                None => (rest, None),
            };
            return Self::AnyDepth {
                name: Segment::parse(name),
                sibling_marker: marker,
            };
        }
        let mut segments: Vec<Segment> = Vec::new();
        let relative = match pattern.strip_prefix(HOME_PREFIX) {
            Some(rest) => {
                segments.extend(HOME_COMPONENTS.iter().map(|component| Segment::parse(component)));
                rest
            }
            None => pattern.trim_start_matches('/'),
        };
        segments.extend(relative.split('/').filter(|part| !part.is_empty()).map(Segment::parse));
        Self::Anchored(segments)
    }

    pub(crate) fn absolute(path: &Path) -> Self {
        Self::Anchored(
            path.components()
                .filter_map(|component| match component {
                    Component::Normal(name) => Some(Segment::Exact(name.to_string_lossy().into_owned())),
                    _ => None,
                })
                .collect(),
        )
    }

    pub(crate) fn folded(&self, case: CaseRule) -> Self {
        match self {
            Self::Anchored(segments) => Self::Anchored(segments.iter().map(|segment| segment.folded(case)).collect()),
            Self::AnyDepth { name, sibling_marker } => Self::AnyDepth {
                name: name.folded(case),
                sibling_marker: sibling_marker.clone(),
            },
        }
    }

    pub(crate) fn anchored_length(&self) -> Option<usize> {
        match self {
            Self::Anchored(segments) => Some(segments.len()),
            Self::AnyDepth { .. } => None,
        }
    }

    pub(crate) fn prefix_matches(&self, components: &[String]) -> bool {
        match self {
            Self::Anchored(segments) => {
                segments.len() > components.len()
                    && segments
                        .iter()
                        .zip(components)
                        .all(|(segment, component)| segment.matches(component))
            }
            Self::AnyDepth { .. } => false,
        }
    }

    pub(crate) fn follows_sibling(&self, name: &str, sibling: &OsStr) -> bool {
        match self {
            Self::AnyDepth {
                name: pattern,
                sibling_marker: Some(marker),
            } => pattern.matches(name) && OsStr::new(marker) == sibling,
            _ => false,
        }
    }

    pub(crate) fn matches(&self, components: &[String], has_sibling: impl Fn(&str) -> bool) -> bool {
        match self {
            Self::Anchored(segments) => {
                segments.len() == components.len()
                    && segments
                        .iter()
                        .zip(components)
                        .all(|(segment, component)| segment.matches(component))
            }
            Self::AnyDepth { name, sibling_marker } => {
                components.last().is_some_and(|last| name.matches(last))
                    && sibling_marker.as_deref().is_none_or(&has_sibling)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PathPattern;
    use crate::name_match::CaseRule;

    fn components(path: &str) -> Vec<String> {
        path.split('/')
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn home_patterns_match_every_user() {
        let pattern = PathPattern::parse("~/Library/Caches/Homebrew").folded(CaseRule::Sensitive);
        assert!(pattern.matches(&components("/Users/igor/Library/Caches/Homebrew"), |_| false));
        assert!(!pattern.matches(&components("/Users/igor/Library/Caches"), |_| false));
    }

    #[test]
    fn any_depth_patterns_require_their_project_marker() {
        let pattern = PathPattern::parse("**/target|Cargo.toml").folded(CaseRule::Sensitive);
        let path = components("/Users/igor/Projects/engram/target");
        assert!(pattern.matches(&path, |marker| marker == "Cargo.toml"));
        assert!(!pattern.matches(&path, |_| false));
    }
}
