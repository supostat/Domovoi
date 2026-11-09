use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use domovoi_index::{ChangedPath, Enrichment, Index, IndexRoot};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet, SimulatorRuntimes, rule_table};
use domovoi_scanner::{ScanOptions, scan};
use domovoi_testkit::{TempTree, app_container, application_bundle};

const APPLICATIONS: &str = "Applications";
const BUNDLE_ID: &str = "com.domovoi.tiny";
const CONTAINER: &str = "Users/tester/Library/Containers/com.domovoi.tiny";
const CONTAINER_CACHES: &str = "Users/tester/Library/Containers/com.domovoi.tiny/Data/Library/Caches";
const CACHE_FILE: &str = "cache.bin";
const LEFTOVER_DATA: &str = "apps.leftover-data";

struct NoSimulators;

impl SimulatorRuntimes for NoSimulators {
    fn device_runtime_missing(&self, _device: &Path) -> bool {
        false
    }
}

struct Volume {
    _tree: TempTree,
    root: PathBuf,
    index: Index,
    rules: RuleSet,
    denylist: Denylist,
    recommendations: Recommendations,
}

impl Volume {
    fn with_a_removed_application() -> Self {
        let tree = TempTree::new().expect("a temp tree is created");
        let root = fs::canonicalize(tree.path()).expect("the root resolves");
        fs::create_dir_all(root.join(APPLICATIONS)).expect("the applications folder is created");
        app_container(&root.join(CONTAINER), BUNDLE_ID).expect("the container is built");
        fs::create_dir_all(root.join(CONTAINER_CACHES)).expect("the container caches are created");
        fs::write(root.join(CONTAINER_CACHES).join(CACHE_FILE), vec![3_u8; 8_192]).expect("a cache is written");
        let report = scan(&root, &ScanOptions::default()).expect("the tree scans");
        let volume = IndexRoot::Volume {
            path: root.to_string_lossy().into_owned(),
        };
        let index = Index::build(&report, volume, &Enrichment::default());
        let rules = RuleSet::new(
            rule_table(&ConfigOverrides::default()),
            CaseRule::Insensitive,
            Box::new(NoSimulators),
        );
        let denylist = Denylist::new(CaseRule::Insensitive).expect("the denylist is prepared");
        let recommendations = Recommendations::build(&index, &rules, &denylist, &BTreeSet::new(), Vec::new());
        Self {
            _tree: tree,
            root,
            index,
            rules,
            denylist,
            recommendations,
        }
    }

    fn install_the_application(&mut self) {
        application_bundle(&self.root.join(APPLICATIONS), "Tiny", BUNDLE_ID, &[]).expect("the application is built");
        self.apply(APPLICATIONS);
    }

    fn apply(&mut self, changed: &str) {
        let request = self.index.delta_request(&[ChangedPath {
            path: self.root.join(changed),
            subtree: false,
        }]);
        let listings = request
            .targets()
            .iter()
            .map(|target| target.read(&ScanOptions::default()).expect("the target reads"))
            .collect();
        let applied = self
            .index
            .apply_delta(&request, listings, &Enrichment::default())
            .expect("the request is current");
        self.recommendations
            .apply_delta(&self.index, &self.rules, &self.denylist, &BTreeSet::new(), &applied);
    }

    fn full_build(&self) -> Recommendations {
        Recommendations::build(&self.index, &self.rules, &self.denylist, &BTreeSet::new(), Vec::new())
    }

    fn cards(&self, recommendations: &Recommendations) -> BTreeMap<&'static str, BTreeSet<String>> {
        recommendations
            .cards
            .iter()
            .map(|card| {
                let paths = card
                    .items
                    .iter()
                    .map(|item| {
                        self.index
                            .path(item.node)
                            .strip_prefix(&self.root)
                            .expect("a card item lies on the volume")
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect();
                (self.rules.rule(card.rule).id, paths)
            })
            .collect()
    }

    fn assert_matches_a_full_build(&self) {
        let full = self.full_build();
        assert_eq!(self.cards(&self.recommendations), self.cards(&full));
        assert_eq!(self.recommendations.totals, full.totals);
    }
}

#[test]
fn installing_the_owner_takes_its_data_out_of_the_leftovers() {
    let mut volume = Volume::with_a_removed_application();
    assert_eq!(
        volume.cards(&volume.recommendations).get(LEFTOVER_DATA),
        Some(&BTreeSet::from([CONTAINER.to_owned()])),
        "the container of the removed application is a leftover"
    );

    volume.install_the_application();

    volume.assert_matches_a_full_build();
}

#[test]
fn a_cache_change_after_the_install_counts_the_cache_once() {
    let mut volume = Volume::with_a_removed_application();
    volume.install_the_application();

    fs::write(volume.root.join(CONTAINER_CACHES).join(CACHE_FILE), vec![4_u8; 16_384]).expect("the cache grows");
    volume.apply(CONTAINER_CACHES);

    volume.assert_matches_a_full_build();
}

#[test]
fn removing_the_owner_puts_its_data_into_the_leftovers() {
    let mut volume = Volume::with_a_removed_application();
    volume.install_the_application();
    let installed = volume.full_build();
    volume.recommendations = installed;
    assert_eq!(volume.cards(&volume.recommendations).get(LEFTOVER_DATA), None);

    fs::remove_dir_all(volume.root.join(APPLICATIONS).join("Tiny.app")).expect("the bundle is removed");
    volume.apply(APPLICATIONS);

    volume.assert_matches_a_full_build();
}
