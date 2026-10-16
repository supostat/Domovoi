use std::ffi::OsStr;

use crate::category::Category;

const ANY: &str = "*";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppDataNaming {
    BundleId,
    Suffixed(&'static str),
}

#[derive(Debug)]
pub(crate) struct PathRule {
    pub(crate) components: &'static [&'static str],
    pub(crate) category: Category,
    pub(crate) app_data: Option<AppDataNaming>,
}

const fn rule(components: &'static [&'static str], category: Category) -> PathRule {
    PathRule {
        components,
        category,
        app_data: None,
    }
}

const fn app_data(components: &'static [&'static str], category: Category, naming: AppDataNaming) -> PathRule {
    PathRule {
        components,
        category,
        app_data: Some(naming),
    }
}

pub(crate) const PATH_RULES: [PathRule; 39] = [
    rule(
        &[
            "Users",
            ANY,
            "Library",
            "Application Support",
            "com.domovoi.app",
            "models",
        ],
        Category::Model,
    ),
    rule(
        &["Users", ANY, "Library", "Application Support", "MobileSync", "Backup"],
        Category::Docs,
    ),
    rule(
        &["Users", ANY, "Library", "Containers", "com.docker.docker"],
        Category::Dev,
    ),
    rule(&["Users", ANY, "Library", "Developer"], Category::Dev),
    app_data(
        &["Users", ANY, "Library", "Caches"],
        Category::Cache,
        AppDataNaming::BundleId,
    ),
    app_data(
        &["Users", ANY, "Library", "Containers"],
        Category::Apps,
        AppDataNaming::BundleId,
    ),
    rule(&["Users", ANY, "Library", "Group Containers"], Category::Apps),
    app_data(
        &["Users", ANY, "Library", "Application Support"],
        Category::Apps,
        AppDataNaming::BundleId,
    ),
    app_data(
        &["Users", ANY, "Library", "Saved Application State"],
        Category::Apps,
        AppDataNaming::Suffixed(".savedState"),
    ),
    app_data(
        &["Users", ANY, "Library", "HTTPStorages"],
        Category::Apps,
        AppDataNaming::BundleId,
    ),
    app_data(
        &["Users", ANY, "Library", "WebKit"],
        Category::Apps,
        AppDataNaming::BundleId,
    ),
    app_data(
        &["Users", ANY, "Library", "Preferences"],
        Category::Apps,
        AppDataNaming::Suffixed(".plist"),
    ),
    rule(&["Users", ANY, "Library", "Mobile Documents"], Category::Docs),
    rule(&["Users", ANY, "Library", "CloudStorage"], Category::Docs),
    rule(&["Users", ANY, "Pictures"], Category::Media),
    rule(&["Users", ANY, "Movies"], Category::Media),
    rule(&["Users", ANY, "Music"], Category::Media),
    rule(&["Users", ANY, "Documents"], Category::Docs),
    rule(&["Users", ANY, "Downloads"], Category::Docs),
    rule(&["Users", ANY, "Desktop"], Category::Docs),
    rule(&["Users", ANY, "Applications"], Category::Apps),
    rule(&["Users", ANY, ".cache"], Category::Cache),
    rule(&["Users", ANY, ".npm"], Category::Cache),
    rule(&["Users", ANY, ".cargo"], Category::Dev),
    rule(&["Users", ANY, ".rustup"], Category::Dev),
    rule(&["Users", ANY], Category::Home),
    rule(&["Users"], Category::Home),
    rule(&["Applications"], Category::Apps),
    rule(&["Library", "Caches"], Category::Cache),
    rule(&["Library", "Developer"], Category::Dev),
    rule(&["Library"], Category::System),
    rule(&["System", "Library", "Caches"], Category::Cache),
    rule(&["System"], Category::System),
    rule(&["private", "var", "folders"], Category::Cache),
    rule(&["private"], Category::System),
    rule(&["opt", "homebrew"], Category::Dev),
    rule(&["usr", "local"], Category::Dev),
    rule(&["usr"], Category::System),
    rule(&["Volumes"], Category::System),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RuleProgress(u64);

impl RuleProgress {
    pub(crate) fn start() -> Self {
        Self((1_u64 << PATH_RULES.len()) - 1)
    }

    pub(crate) fn descend(self, depth: usize, name: &OsStr) -> Self {
        let mut still_matching = 0;
        for (index, path_rule) in PATH_RULES.iter().enumerate() {
            let bit = 1_u64 << index;
            if self.0 & bit == 0 {
                continue;
            }
            let Some(expected) = path_rule.components.get(depth - 1) else {
                continue;
            };
            if *expected == ANY || OsStr::new(expected) == name {
                still_matching |= bit;
            }
        }
        Self(still_matching)
    }

    pub(crate) fn completed(self, depth: usize) -> Option<&'static PathRule> {
        PATH_RULES
            .iter()
            .enumerate()
            .find(|(index, path_rule)| self.0 & (1 << index) != 0 && path_rule.components.len() == depth)
            .map(|(_, path_rule)| path_rule)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::RuleProgress;
    use crate::category::Category;

    fn category_of(path: &[&str]) -> Option<Category> {
        let mut progress = RuleProgress::start();
        let mut category = None;
        for (index, component) in path.iter().enumerate() {
            progress = progress.descend(index + 1, OsStr::new(component));
            if let Some(path_rule) = progress.completed(index + 1) {
                category = Some(path_rule.category);
            }
        }
        category
    }

    #[test]
    fn the_most_specific_rule_on_a_path_wins() {
        assert_eq!(
            category_of(&["Users", "igor", "Library", "Caches", "Homebrew"]),
            Some(Category::Cache)
        );
        assert_eq!(
            category_of(&["Users", "igor", "Library", "Containers", "com.docker.docker", "Data"]),
            Some(Category::Dev)
        );
        assert_eq!(
            category_of(&[
                "Users",
                "igor",
                "Library",
                "Application Support",
                "MobileSync",
                "Backup",
                "x"
            ]),
            Some(Category::Docs)
        );
        assert_eq!(
            category_of(&["Library", "Caches", "com.apple.x"]),
            Some(Category::Cache)
        );
        assert_eq!(category_of(&["Library", "Audio"]), Some(Category::System));
    }
}
