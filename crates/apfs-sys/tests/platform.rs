mod temp_dir;

use std::ffi::OsStr;
use std::fs::{self, File};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use domovoi_apfs_sys::sys::IOPOL_UTILITY;
use domovoi_apfs_sys::{
    AccessError, AttributeSet, BulkListing, container_capacity, data_volume, firmlinks, gen_count, link_id, mkdir_at,
    mounted_volumes, open_at, open_search, os_build_version, pids_with_open, private_size, process_path,
    raise_nofile_limit, real_dev, running_bundle_roots, set_thread_disk_policy_utility, set_vfs_policies, snapshots,
    thread_disk_policy, user_access, volume_capabilities, volume_used, volume_uuid,
};
use temp_dir::TempDir;

const KIBIBYTE: u64 = 1024;
const MINIMUM_FIRMLINKS: usize = 18;
const GUI_DEFAULT_FILE_LIMIT: u64 = 256;

fn data() -> PathBuf {
    data_volume().expect("the root data volume is mounted")
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .expect("system tool runs");
    assert!(output.status.success(), "{program} {arguments:?} failed");
    String::from_utf8(output.stdout).expect("tool output is UTF-8")
}

fn df_used_bytes(volume: &Path) -> u64 {
    let report = command_output("/bin/df", &["-k", volume.to_str().expect("volume path is UTF-8")]);
    let used_kibibytes: u64 = report
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(2))
        .and_then(|used| used.parse().ok())
        .expect("df reports the used column");
    used_kibibytes * KIBIBYTE
}

fn diskutil_container_free(volume: &Path) -> u64 {
    let plist = command_output(
        "/usr/sbin/diskutil",
        &["info", "-plist", volume.to_str().expect("volume path is UTF-8")],
    );
    let after_key = plist
        .split_once("<key>APFSContainerFree</key>")
        .expect("diskutil reports APFSContainerFree")
        .1;
    let value = after_key
        .split_once("<integer>")
        .and_then(|(_, rest)| rest.split_once("</integer>"))
        .expect("APFSContainerFree is an integer")
        .0;
    value.parse().expect("APFSContainerFree parses")
}

fn assert_between(value: u64, first: u64, second: u64, tolerance: u64, what: &str) {
    let low = first.min(second).saturating_sub(tolerance);
    let high = first.max(second) + tolerance;
    assert!(
        (low..=high).contains(&value),
        "{what}: {value} is outside {low}..={high}"
    );
}

#[test]
fn data_volume_differs_from_root_by_real_device_with_equal_st_dev() {
    let data = data();
    assert_eq!(data, Path::new("/System/Volumes/Data"));
    let root_metadata = fs::metadata("/").expect("root metadata");
    let data_metadata = fs::metadata(&data).expect("data metadata");
    assert_eq!(root_metadata.dev(), data_metadata.dev());
    assert_ne!(
        real_dev(Path::new("/")).expect("root real dev"),
        real_dev(&data).expect("data real dev")
    );
}

#[test]
fn snapshots_are_listed_without_root() {
    let volume_root = OwnedFd::from(File::open(data()).expect("data volume root opens"));
    let listed = snapshots(volume_root.as_fd()).expect("fs_snapshot_list succeeds without root");
    for snapshot in listed {
        assert!(!snapshot.name.is_empty());
    }
}

#[test]
fn unix_permissions_surface_as_eacces() {
    let error = File::open("/private/etc/master.passwd").expect_err("master.passwd is root-only");
    assert!(matches!(AccessError::from(error), AccessError::Unix));
}

#[test]
fn firmlinks_map_user_paths_onto_the_data_volume() {
    let firmlinks = firmlinks().expect("firmlinks are read");
    assert!(firmlinks.len() >= MINIMUM_FIRMLINKS, "{} firmlinks", firmlinks.len());
    assert!(
        firmlinks
            .iter()
            .any(|firmlink| firmlink.path == Path::new("/Users") && firmlink.data_relative == Path::new("Users"))
    );
}

#[test]
fn data_volume_supports_clones_and_rename_swap() {
    let capabilities = volume_capabilities(&data()).expect("capabilities are read");
    assert!(capabilities.clone);
    assert!(capabilities.rename_swap);
}

#[test]
fn search_only_directory_opens_for_search_but_refuses_listing() {
    let temp = TempDir::new("search-only");
    let root = temp.open();
    let name = OsStr::new("search-only");
    mkdir_at(root.as_fd(), name, 0o100).expect("search-only directory is created");
    let listing_attempt = open_at(
        root.as_fd(),
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        0,
    );
    let searchable = open_search(root.as_fd(), name).expect("search-only directory opens for search");
    let first = BulkListing::new(searchable.as_fd(), AttributeSet::Extended).next();
    fs::set_permissions(temp.path().join(name), fs::Permissions::from_mode(0o755)).expect("permissions are restored");

    assert!(matches!(
        listing_attempt.map_err(AccessError::from),
        Err(AccessError::Unix)
    ));
    let error = first
        .expect("listing yields an outcome")
        .expect_err("listing is refused");
    assert_eq!(error.raw_os_error(), Some(libc::EBADF));
}

#[test]
fn launchd_is_pid_one() {
    assert_eq!(
        process_path(1).expect("launchd path is readable"),
        Path::new("/sbin/launchd")
    );
}

#[test]
fn running_bundle_roots_are_application_bundles() {
    for root in running_bundle_roots().expect("process list is readable") {
        assert_eq!(root.extension(), Some(OsStr::new("app")), "{}", root.display());
    }
}

#[test]
fn own_open_file_is_found_by_path() {
    let temp = TempDir::new("open-file");
    let path = temp.path().join("held");
    let _held = File::create(&path).expect("file is created and held open");
    let pid = libc::pid_t::try_from(std::process::id()).expect("pid fits in pid_t");
    assert!(pids_with_open(&path).expect("open files are listed").contains(&pid));
}

#[test]
fn unshared_file_keeps_its_whole_allocation_private() {
    let temp = TempDir::new("private-size");
    let path = temp.path().join("owned");
    fs::write(&path, vec![7_u8; 64 * 1024]).expect("file is written");
    let allocated = fs::metadata(&path).expect("metadata").blocks() * 512;
    assert_eq!(private_size(&path).expect("private size is read"), allocated);
    gen_count(&path).expect("generation count is read");
}

#[test]
fn volume_used_matches_df() {
    let data = data();
    let before = df_used_bytes(&data);
    let used = volume_used(&data).expect("used space is read");
    let after = df_used_bytes(&data);
    assert_between(used, before, after, KIBIBYTE, "ATTR_VOL_SPACEUSED against df -k Used");
}

#[test]
fn container_available_matches_diskutil() {
    let data = data();
    let before = diskutil_container_free(&data);
    let capacity = container_capacity(&data).expect("container capacity is read");
    let after = diskutil_container_free(&data);
    assert!(capacity.size > capacity.available);
    assert_between(
        capacity.available,
        before,
        after,
        KIBIBYTE,
        "ATTR_VOL_SPACEAVAIL against APFSContainerFree",
    );
}

#[test]
fn thread_disk_policy_becomes_utility() {
    let policy = std::thread::spawn(|| {
        set_thread_disk_policy_utility().expect("thread policy is set");
        thread_disk_policy().expect("thread policy is read")
    })
    .join()
    .expect("policy thread finishes");
    assert_eq!(policy, IOPOL_UTILITY);
}

#[test]
fn process_policies_and_file_limit_are_raised() {
    set_vfs_policies().expect("VFS policies are set");
    assert!(raise_nofile_limit().expect("file limit is raised") > GUI_DEFAULT_FILE_LIMIT);
}

fn plist_string(plist: &str, key: &str) -> String {
    plist
        .split_once(&format!("<key>{key}</key>"))
        .and_then(|(_, rest)| rest.split_once("<string>"))
        .and_then(|(_, rest)| rest.split_once("</string>"))
        .map_or_else(|| panic!("diskutil reports {key}"), |(value, _)| value.to_owned())
}

#[test]
fn volume_identity_matches_diskutil() {
    let data = data();
    let plist = command_output(
        "/usr/sbin/diskutil",
        &["info", "-plist", data.to_str().expect("volume path is UTF-8")],
    );
    assert_eq!(
        volume_uuid(&data).expect("volume UUID is read"),
        plist_string(&plist, "VolumeUUID")
    );
    let data_mount = mounted_volumes()
        .expect("mount table is read")
        .into_iter()
        .find(|volume| volume.root_data)
        .expect("the root data volume is mounted");
    assert_eq!(data_mount.mount_point, data);
    assert_eq!(data_mount.file_system, "apfs");
    assert!(!data_mount.read_only);
    assert_eq!(
        data_mount.device,
        Path::new("/dev").join(plist_string(&plist, "DeviceIdentifier"))
    );
}

#[test]
fn os_build_matches_sw_vers() {
    let build = command_output("/usr/bin/sw_vers", &["-buildVersion"]);
    assert_eq!(os_build_version().expect("build version is read"), build.trim());
}

#[test]
fn access_and_link_identity_are_read_per_path() {
    let temp = TempDir::new("access");
    let path = temp.path().join("entry");
    fs::write(&path, b"x").expect("file is written");
    let writable = libc::c_uint::try_from(libc::W_OK).expect("W_OK fits");
    assert_ne!(user_access(temp.path()).expect("access is read") & writable, 0);
    assert_eq!(user_access(Path::new("/System")).expect("access is read") & writable, 0);
    let first = link_id(&path).expect("link id is read");
    fs::hard_link(&path, temp.path().join("second")).expect("link is created");
    assert_ne!(link_id(&temp.path().join("second")).expect("link id is read"), first);
}
