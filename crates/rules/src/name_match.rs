use std::ffi::OsStr;

use objc2_foundation::NSString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseRule {
    Sensitive,
    Insensitive,
}

pub(crate) fn normalized(name: &OsStr, case: CaseRule) -> String {
    let text = name.to_string_lossy();
    let composed = if text.is_ascii() {
        text.into_owned()
    } else {
        NSString::from_str(&text)
            .precomposedStringWithCanonicalMapping()
            .to_string()
    };
    match case {
        CaseRule::Sensitive => composed,
        CaseRule::Insensitive => composed.to_lowercase(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Segment {
    Exact(String),
    Glob(Vec<String>),
    Any,
}

impl Segment {
    pub(crate) fn parse(text: &str) -> Self {
        if text == "*" {
            Self::Any
        } else if text.contains('*') {
            Self::Glob(text.split('*').map(str::to_owned).collect())
        } else {
            Self::Exact(text.to_owned())
        }
    }

    pub(crate) fn folded(&self, case: CaseRule) -> Self {
        let fold = |text: &str| normalized(OsStr::new(text), case);
        match self {
            Self::Any => Self::Any,
            Self::Exact(expected) => Self::Exact(fold(expected)),
            Self::Glob(parts) => Self::Glob(parts.iter().map(|part| fold(part)).collect()),
        }
    }

    pub(crate) fn matches(&self, name: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => expected == name,
            Self::Glob(parts) => glob_matches(parts, name),
        }
    }
}

fn glob_matches(parts: &[String], name: &str) -> bool {
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
        return false;
    };
    if !name.starts_with(first.as_str()) || !name.ends_with(last.as_str()) || name.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle.as_str()) {
            Some(position) => rest = &rest[position + middle.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{CaseRule, Segment, normalized};

    #[test]
    fn names_compare_after_nfc_and_optional_case_folding() {
        let decomposed = OsStr::new("Cafe\u{301}");
        assert_eq!(normalized(decomposed, CaseRule::Sensitive), "Caf\u{e9}");
        let folded = normalized(OsStr::new("Homebrew"), CaseRule::Insensitive);
        assert!(
            Segment::parse("homebrew")
                .folded(CaseRule::Insensitive)
                .matches(&folded)
        );
        let exact = normalized(OsStr::new("homebrew"), CaseRule::Sensitive);
        assert!(!Segment::parse("Homebrew").folded(CaseRule::Sensitive).matches(&exact));
        assert!(Segment::parse("*.dmg").matches("installer.dmg"));
        assert!(!Segment::parse("*.dmg").matches("notes.txt"));
        assert!(Segment::parse("*.com.apple.*").matches("UBF8T346G9.com.apple.notes"));
        assert!(!Segment::parse("*.com.apple.*").matches("com.apple"));
    }
}
