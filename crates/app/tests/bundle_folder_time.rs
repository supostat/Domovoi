use std::fs;
use std::path::Path;
use std::time::SystemTime;

const BUNDLE: &str = "../../target/bundle/DOMOVOI.app";
const ASSEMBLED_FILES: [&str; 3] = [
    "Contents/Info.plist",
    "Contents/Resources/AppIcon.icns",
    "Contents/MacOS/domovoi",
];

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn bundle_folder_is_not_older_than_the_files_its_assembly_wrote() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join(BUNDLE);
    assert!(
        bundle.is_dir(),
        "{} is missing: cargo xtask bundle --profile adhoc builds it",
        bundle.display()
    );
    let folder = modified(&bundle);
    for relative in ASSEMBLED_FILES {
        let file = bundle.join(relative);
        assert!(
            folder >= modified(&file),
            "{} is older than {}: the system keeps showing the previous icon of a bundle whose folder time did not move",
            bundle.display(),
            file.display()
        );
    }
}
