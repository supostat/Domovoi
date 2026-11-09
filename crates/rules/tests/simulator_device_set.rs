mod synthetic_tree;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use domovoi_rules::{CaseRule, ConfigOverrides, RuleSet, SimulatorRuntimes, rule_table};
use synthetic_tree::{data_volume_index, directory, file, path_tree};

const CORE_SIMULATOR: &str = "/Users/igor/Library/Developer/CoreSimulator";

struct OnlyCurrentDeviceInstalled;

impl SimulatorRuntimes for OnlyCurrentDeviceInstalled {
    fn device_runtime_missing(&self, device: &Path) -> bool {
        device.file_name().is_some_and(|name| name != "CURRENT-DEVICE")
    }
}

#[test]
fn the_device_set_beside_simulator_folders_is_not_a_simulator() {
    let index = data_volume_index(vec![path_tree(
        CORE_SIMULATOR,
        directory(
            "Devices",
            vec![
                directory("OLD-DEVICE", vec![file("device.plist")]),
                directory("CURRENT-DEVICE", vec![file("device.plist")]),
                file("device_set.plist"),
            ],
        ),
    )]);
    let rules = RuleSet::new(
        rule_table(&ConfigOverrides::default()),
        CaseRule::Insensitive,
        Box::new(OnlyCurrentDeviceInstalled),
    );

    let claims: BTreeMap<PathBuf, &str> = rules
        .claims(&index)
        .into_iter()
        .map(|claim| (index.path(claim.node), rules.rule(claim.rule).id))
        .collect();

    let devices = Path::new(CORE_SIMULATOR).join("Devices");
    assert_eq!(
        claims,
        BTreeMap::from([
            (devices.join("CURRENT-DEVICE"), "xcode.simulators"),
            (devices.join("OLD-DEVICE"), "xcode.simulators-unavailable"),
        ]),
        "only device folders are simulators"
    );
}
