mod synthetic_tree;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use domovoi_apfs_sys::{data_volume, volume_capabilities};
use domovoi_index::{Index, SafetyLevel};
use domovoi_rules::{CaseRule, ConfigOverrides, RuleSet, SimulatorRuntimes, rule_table};
use domovoi_testkit::{ImageVolume, TempTree, Visibility, VolumeFormat};
use synthetic_tree::{data_volume_index, directory, file, merge, path_tree};

struct OnlyOldDevicesMissing;

impl SimulatorRuntimes for OnlyOldDevicesMissing {
    fn device_runtime_missing(&self, device: &Path) -> bool {
        device.file_name().is_some_and(|name| name == "OLD-DEVICE")
    }
}

fn case_of(volume: &Path) -> CaseRule {
    if volume_capabilities(volume)
        .expect("capabilities are read")
        .case_sensitive
    {
        CaseRule::Sensitive
    } else {
        CaseRule::Insensitive
    }
}

fn rule_set(overrides: &ConfigOverrides, case: CaseRule) -> RuleSet {
    RuleSet::new(rule_table(overrides), case, Box::new(OnlyOldDevicesMissing))
}

fn claimed(index: &Index, rules: &RuleSet) -> BTreeMap<PathBuf, &'static str> {
    rules
        .claims(index)
        .into_iter()
        .map(|claim| (index.path(claim.node), rules.rule(claim.rule).id))
        .collect()
}

fn fixture_for_every_rule() -> (Vec<(&'static str, &'static str)>, Index) {
    let fixtures = vec![
        ("/Users/igor/Library/Developer/Xcode/DerivedData", "xcode.derived-data"),
        (
            "/Users/igor/Library/Developer/Xcode/iOS DeviceSupport",
            "xcode.device-support",
        ),
        (
            "/Users/igor/Library/Developer/CoreSimulator/Devices/OLD-DEVICE",
            "xcode.simulators-unavailable",
        ),
        (
            "/Users/igor/Library/Developer/CoreSimulator/Devices/CURRENT-DEVICE",
            "xcode.simulators",
        ),
        (
            "/Users/igor/Library/Developer/CoreSimulator/Caches/dyld",
            "xcode.dyld-cache",
        ),
        ("/Users/igor/Library/Caches/com.apple.dt.Xcode", "xcode.caches"),
        ("/Users/igor/Library/Developer/Xcode/Archives", "xcode.archives"),
        (
            "/Library/Developer/CoreSimulator/Volumes/iOS_23A343",
            "xcode.simulator-runtimes",
        ),
        ("/Users/igor/Library/Containers/com.docker.docker/Data/vms", "docker.vm"),
        ("/Users/igor/.orbstack", "orbstack"),
        (
            "/Users/igor/Library/Application Support/MobileSync/Backup/00008110",
            "ios.backups",
        ),
        ("/Users/igor/Projects/engram/target", "cargo.target"),
        ("/Users/igor/Projects/site/node_modules", "dev.deps"),
        ("/Users/igor/Projects/tool/.zig-cache", "dev.deps"),
        ("/Users/igor/Downloads/Figma.dmg", "downloads.installers"),
        ("/Users/igor/Downloads/Driver.pkg", "downloads.installers"),
        ("/Users/igor/Library/Caches/Homebrew", "cache.homebrew"),
        ("/Users/igor/.npm/_cacache", "cache.npm"),
        ("/Users/igor/Library/pnpm/store", "cache.pnpm-store"),
        ("/Users/igor/Library/Caches/pnpm", "cache.pnpm-metadata"),
        ("/Users/igor/Library/Caches/Yarn", "cache.yarn"),
        ("/Users/igor/.yarn/berry/cache", "cache.yarn-berry"),
        ("/Users/igor/.bun/install/cache", "cache.bun"),
        ("/Users/igor/.cargo/registry", "cache.cargo-registry"),
        ("/Users/igor/.cargo/git", "cache.cargo-git"),
        ("/Users/igor/Library/Caches/pip", "cache.pip"),
        ("/Users/igor/.cache/uv", "cache.uv"),
        ("/Users/igor/Library/Caches/pypoetry/cache", "cache.poetry"),
        ("/Users/igor/Library/Caches/pypoetry/virtualenvs", "cache.poetry-venvs"),
        ("/Users/igor/.gradle/caches", "cache.gradle"),
        ("/Users/igor/.m2/repository", "cache.maven"),
        ("/Users/igor/Library/Caches/CocoaPods", "cache.cocoapods"),
        ("/Users/igor/.cocoapods/repos", "cache.cocoapods-specs"),
        ("/Users/igor/Library/Caches/org.swift.swiftpm", "cache.swiftpm"),
        ("/Users/igor/Library/Caches/go-build", "cache.go-build"),
        ("/Users/igor/go/pkg/mod", "cache.go-mod"),
        ("/Users/igor/Library/Caches/composer", "cache.composer"),
        ("/Users/igor/.local/share/NuGet/http-cache", "cache.nuget-http"),
        ("/Users/igor/.nuget/packages", "cache.nuget-packages"),
        ("/Users/igor/Library/Caches/org.carthage.CarthageKit", "cache.carthage"),
        ("/Users/igor/Library/Caches/Google/Chrome", "cache.chrome"),
        ("/Users/igor/Library/Application Support/Slack/Cache", "cache.slack"),
        (
            "/Users/igor/Library/Containers/com.example.Notes/Data/Library/Caches",
            "cache.app.generic",
        ),
        ("/Users/igor/Library/Caches/com.example.viewer", "caches.user"),
    ];
    let mut roots = Vec::new();
    for (path, _) in &fixtures {
        let leaf_name = Path::new(path)
            .file_name()
            .expect("fixture has a name")
            .to_string_lossy()
            .into_owned();
        let parent = Path::new(path)
            .parent()
            .expect("fixture has a parent")
            .to_string_lossy()
            .into_owned();
        let mut leaf = directory(&leaf_name, vec![file("payload")]);
        let installer = Path::new(&leaf_name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("dmg") || extension.eq_ignore_ascii_case("pkg"));
        if installer {
            leaf = file(&leaf_name);
        }
        roots.push(path_tree(&parent, leaf));
    }
    roots.push(path_tree("/Users/igor/Projects/engram", file("Cargo.toml")));
    roots.push(path_tree("/Users/igor/Projects/site", file("package.json")));
    roots.push(path_tree("/Users/igor/Projects/tool", file("build.zig")));
    (fixtures, data_volume_index(merge(roots)))
}

#[test]
fn every_rule_claims_its_fixture_with_a_calm_explanation() {
    let (fixtures, index) = fixture_for_every_rule();
    let rules = rule_set(&ConfigOverrides::default(), CaseRule::Insensitive);
    let claims = claimed(&index, &rules);
    for (path, id) in &fixtures {
        assert_eq!(claims.get(Path::new(path)).copied(), Some(*id), "{path}");
    }
    let covered: BTreeSet<&str> = fixtures.iter().map(|(_, id)| *id).collect();
    for rule in rules.rules() {
        assert!(covered.contains(rule.id), "{} has no fixture", rule.id);
        assert!(!rule.explanation.contains('!'), "{} raises its voice", rule.id);
    }
    let runtimes = rules
        .rules()
        .iter()
        .find(|rule| rule.id == "xcode.simulator-runtimes")
        .expect("rule exists");
    assert!(runtimes.is_info_only());
    let unavailable = rules
        .rules()
        .iter()
        .find(|rule| rule.id == "xcode.simulators-unavailable")
        .expect("rule exists");
    assert_eq!(unavailable.level, SafetyLevel::Safe);
}

#[test]
fn the_volume_decides_whether_case_matters() {
    let index = data_volume_index(vec![path_tree(
        "/Users/igor/library/caches/homebrew",
        file("bottle.tar.gz"),
    )]);
    let data_case = case_of(&data_volume().expect("data volume is mounted"));
    let on_data = claimed(&index, &rule_set(&ConfigOverrides::default(), data_case));
    assert_eq!(
        on_data.get(Path::new("/Users/igor/library/caches/homebrew")).copied(),
        Some("cache.homebrew")
    );
    let mut image = ImageVolume::create(VolumeFormat::CaseSensitiveApfs, Visibility::Hidden).expect("image attaches");
    let image_case = case_of(image.mount_point());
    image.detach().expect("image detaches");
    assert_eq!(image_case, CaseRule::Sensitive);
    assert!(claimed(&index, &rule_set(&ConfigOverrides::default(), image_case)).is_empty());
}

#[test]
fn configured_locations_match_even_when_names_are_decomposed() {
    let home = TempTree::new().expect("temp home is created");
    fs::create_dir_all(home.path().join(".cargo")).expect("cargo directory is created");
    fs::write(
        home.path().join(".cargo/config.toml"),
        "[build]\ntarget-dir = \"/Users/igor/builds/target\"\n",
    )
    .expect("cargo config is written");
    fs::create_dir_all(home.path().join(".config/pnpm")).expect("pnpm directory is created");
    fs::write(
        home.path().join(".config/pnpm/rc"),
        "store-dir=/Users/igor/Caf\u{e9}/store\n",
    )
    .expect("pnpm rc is written");
    let overrides = ConfigOverrides::read(home.path()).expect("configs are read");
    let index = data_volume_index(merge(vec![
        path_tree("/Users/igor/builds/target", file("app")),
        path_tree("/Users/igor/Cafe\u{301}/store", file("blob")),
    ]));
    let claims = claimed(&index, &rule_set(&overrides, CaseRule::Insensitive));
    assert_eq!(
        claims.get(Path::new("/Users/igor/builds/target")).copied(),
        Some("cargo.target")
    );
    assert_eq!(
        claims.get(Path::new("/Users/igor/Cafe\u{301}/store")).copied(),
        Some("cache.pnpm-store")
    );
}
