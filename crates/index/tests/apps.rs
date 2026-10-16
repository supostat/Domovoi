mod synthetic_tree;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use domovoi_index::{Enrichment, Index, Installation, InstalledApplications, NodeId, Timestamp};
use synthetic_tree::{data_volume, directory, file, opened_file};

fn at(index: &Index, relative: &str) -> NodeId {
    Path::new(relative).components().fold(index.root(), |node, name| {
        index
            .child_named(node, name.as_os_str())
            .unwrap_or_else(|| panic!("{relative} is indexed"))
    })
}

#[test]
fn containers_without_an_application_are_leftovers() {
    let opened = UNIX_EPOCH + Duration::from_secs(1_720_000_000);
    let (report, root) = data_volume(vec![directory(
        "Users",
        vec![directory(
            "igor",
            vec![directory(
                "Library",
                vec![
                    directory(
                        "Containers",
                        vec![
                            directory("com.fake.app", vec![file("cache.db", 400)]),
                            directory("com.apple.Notes", vec![file("notes.db", 20)]),
                            directory("com.tinyspeck.slackmacgap", vec![opened_file("history", 50, opened)]),
                        ],
                    ),
                    directory("Preferences", vec![file("com.fake.app.plist", 1)]),
                ],
            )],
        )],
    )]);
    let slack = PathBuf::from("/Applications/Slack.app");
    let enrichment = Enrichment {
        applications: InstalledApplications::new(HashMap::from([(
            "com.tinyspeck.slackmacgap".to_owned(),
            slack.clone(),
        )])),
        spotlight_last_used: vec![(slack.clone(), opened)],
        ..Enrichment::default()
    };
    let index = Index::build(&report, root, &enrichment);

    let fake = index
        .node(at(&index, "Users/igor/Library/Containers/com.fake.app/cache.db"))
        .app;
    let fake_app = index.app(fake).expect("the container has an owner");
    assert_eq!(fake_app.bundle_id, "com.fake.app");
    assert!(fake_app.is_leftover());
    let preferences = index
        .node(at(&index, "Users/igor/Library/Preferences/com.fake.app.plist"))
        .app;
    assert_eq!(preferences, fake, "preferences of the same bundle share one owner");

    let notes = index
        .node(at(&index, "Users/igor/Library/Containers/com.apple.Notes"))
        .app;
    assert_eq!(index.app(notes).expect("owner").installation, Installation::System);

    let slack_container = at(&index, "Users/igor/Library/Containers/com.tinyspeck.slackmacgap");
    let slack_app = index.app(index.node(slack_container).app).expect("owner");
    assert_eq!(
        slack_app.installation,
        Installation::Installed {
            bundle_path: slack.to_string_lossy().into_owned()
        }
    );
    assert_eq!(slack_app.last_opened, Timestamp::from_system_time(opened));
    assert_eq!(
        index.last_activity(slack_container),
        Timestamp::from_system_time(opened)
    );
    assert!(!index.node(at(&index, "Users/igor/Library")).app.is_some());
}
