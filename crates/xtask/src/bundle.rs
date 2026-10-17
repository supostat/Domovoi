use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::build_outputs::BuildOutputs;
use crate::bundle_layout::{BundleLayout, CATALOG, DIAGRAM_METALLIB, SERVICE_METALLIBS};
use crate::bundle_profile::BundleProfile;
use crate::copy_tree::copy_tree;
use crate::finding::Finding;
use crate::notarization::publish;
use crate::property_list;
use crate::signing::Signing;
use crate::sparkle_distribution::SparkleDistribution;
use crate::workspace;
use crate::workspace_version;

const APP_TEMPLATE: &str = "crates/app/bundle/Info.plist";
const SERVICE_TEMPLATE: &str = "crates/inference/bundle/Info.plist";
const SERVICE_ENTITLEMENTS: &str = "crates/inference/bundle/domovoi-inference.entitlements";
const FONTS: &str = "assets/fonts";
const FONT_EXTENSION: &str = "ttf";
const FONT_LICENSE: &str = "OFL.txt";
const LLAMA_LICENSE: &str = "vendor/llama.cpp/LICENSE";
const BUNDLE_DIRECTORY: &str = "target/bundle";
const SHORT_VERSION_KEY: &str = "CFBundleShortVersionString";
const VERSION_KEY: &str = "CFBundleVersion";
const TEAM_KEY: &str = "DOMOVOITeamIdentifier";
const SPARKLE_SKIPPED: [&str; 1] = ["XPCServices"];
const SPARKLE_HELPERS: [&str; 2] = ["Versions/B/Autoupdate", "Versions/B/Updater.app"];

pub(crate) fn run(profile: &str) -> io::Result<Vec<Finding>> {
    let profile = BundleProfile::from_name(profile)?;
    let root = workspace::root();
    let version = workspace_version::read(&root)?;
    let app = assemble(&root, profile, &version)?;
    println!("{} is assembled and signed", app.display());
    if profile == BundleProfile::Release {
        let published = publish(&root, &app, &version)?;
        println!(
            "{} and {} are notarized",
            published.zip.display(),
            published.disk_image.display()
        );
    }
    Ok(Vec::new())
}

pub(crate) fn assemble(root: &Path, profile: BundleProfile, version: &str) -> io::Result<PathBuf> {
    let signing = profile.signing()?;
    let sparkle = SparkleDistribution::ensure(root)?;
    let outputs = BuildOutputs::build_release(root, profile.cargo_features())?;
    let layout = BundleLayout::in_directory(&root.join(BUNDLE_DIRECTORY));
    place_executables(&layout, &outputs)?;
    place_property_lists(root, &layout, version, &signing)?;
    place_resources(root, &layout, &outputs, &sparkle)?;
    copy_tree(&sparkle.framework(), &layout.sparkle(), &SPARKLE_SKIPPED)?;
    sign(root, &layout, &signing)?;
    Ok(layout.app)
}

fn place_executables(layout: &BundleLayout, outputs: &BuildOutputs) -> io::Result<()> {
    copy_into(
        &BuildOutputs::require(outputs.app.as_ref(), "the application executable")?,
        &layout.executable(),
    )?;
    copy_into(
        &BuildOutputs::require(outputs.service.as_ref(), "the service executable")?,
        &layout.service_executable(),
    )
}

fn place_property_lists(root: &Path, layout: &BundleLayout, version: &str, signing: &Signing) -> io::Result<()> {
    for (template, target) in [(APP_TEMPLATE, layout.info()), (SERVICE_TEMPLATE, layout.service_info())] {
        copy_into(&root.join(template), &target)?;
        property_list::set_string(&target, SHORT_VERSION_KEY, version)?;
        property_list::set_string(&target, VERSION_KEY, version)?;
    }
    match &signing.team {
        Some(team) => property_list::set_string(&layout.service_info(), TEAM_KEY, team),
        None => property_list::remove(&layout.service_info(), TEAM_KEY),
    }
}

fn place_resources(
    root: &Path,
    layout: &BundleLayout,
    outputs: &BuildOutputs,
    sparkle: &SparkleDistribution,
) -> io::Result<()> {
    for family in fs::read_dir(root.join(FONTS))? {
        let family = family?.path();
        if !family.is_dir() {
            continue;
        }
        let name = family
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "a font family has no name"))?
            .to_owned();
        for file in fs::read_dir(&family)? {
            let file = file?.path();
            if file.extension().is_some_and(|extension| extension == FONT_EXTENSION) {
                copy_into(
                    &file,
                    &layout.fonts().join(file.file_name().expect("a font file has a name")),
                )?;
            }
        }
        copy_into(
            &family.join(FONT_LICENSE),
            &layout.licenses().join(format!("{name}-{FONT_LICENSE}")),
        )?;
    }
    copy_into(&sparkle.license(), &layout.licenses().join("Sparkle-LICENSE.txt"))?;
    copy_into(
        &root.join(LLAMA_LICENSE),
        &layout.licenses().join("llama.cpp-LICENSE.txt"),
    )?;
    copy_into(&root.join("assets").join(CATALOG), &layout.resources().join(CATALOG))?;
    copy_into(
        &BuildOutputs::require(outputs.diagram_metallib.as_ref(), "the diagram shaders")?,
        &layout.resources().join(DIAGRAM_METALLIB),
    )?;
    let metallibs = BuildOutputs::require(outputs.metallib_directory.as_ref(), "the llama.cpp shaders")?;
    for metallib in SERVICE_METALLIBS {
        copy_into(&metallibs.join(metallib), &layout.service_resources().join(metallib))?;
    }
    Ok(())
}

fn sign(root: &Path, layout: &BundleLayout, signing: &Signing) -> io::Result<()> {
    for helper in SPARKLE_HELPERS {
        signing.sign(&layout.sparkle().join(helper), None)?;
    }
    signing.sign(&layout.sparkle(), None)?;
    signing.sign(&layout.service(), Some(&root.join(SERVICE_ENTITLEMENTS)))?;
    signing.sign(&layout.app, None)
}

fn copy_into(source: &Path, target: &Path) -> io::Result<()> {
    if let Some(directory) = target.parent() {
        fs::create_dir_all(directory)?;
    }
    fs::copy(source, target).map(drop).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("{} → {}: {error}", source.display(), target.display()),
        )
    })
}
