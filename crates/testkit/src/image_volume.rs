use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::image_backend::ImageBackend;
use crate::system_tool::run;
use crate::temp_tree::TempTree;
use crate::test_timeout::test_timeout;

const IMAGE_SIZE: &str = "32m";
const VOLUME_NAME: &str = "DOMOVOI";
const IMAGE_FILE: &str = "volume.dmg";
const MOUNT_DIRECTORY: &str = "mount";
const FIRST_DETACH_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeFormat {
    Apfs,
    CaseSensitiveApfs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Hidden,
    Browsable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Access {
    ReadWrite,
    ReadOnly,
}

#[derive(Debug)]
pub struct ImageVolume {
    backend: ImageBackend,
    image: PathBuf,
    device: Option<String>,
    mount_point: PathBuf,
    workspace: TempTree,
}

impl ImageVolume {
    pub fn create(format: VolumeFormat, visibility: Visibility) -> io::Result<Self> {
        let backend = ImageBackend::from_environment();
        let workspace = TempTree::new()?;
        let image = workspace.path().join(IMAGE_FILE);
        let mount_point = workspace.path().join(MOUNT_DIRECTORY);
        std::fs::create_dir(&mount_point)?;
        match backend {
            ImageBackend::Hdiutil => create_with_hdiutil(&image, format)?,
            ImageBackend::DiskutilImage => create_with_diskutil(&image, format)?,
        }
        let mut volume = Self {
            backend,
            image,
            device: None,
            mount_point,
            workspace,
        };
        volume.attach(visibility)?;
        Ok(volume)
    }

    pub fn attach(&mut self, visibility: Visibility) -> io::Result<()> {
        self.attach_with(visibility, Access::ReadWrite)
    }

    pub fn attach_read_only(&mut self) -> io::Result<()> {
        self.attach_with(Visibility::Hidden, Access::ReadOnly)
    }

    fn attach_with(&mut self, visibility: Visibility, access: Access) -> io::Result<()> {
        if self.device.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "image is already attached",
            ));
        }
        let attach_output = match self.backend {
            ImageBackend::Hdiutil => attach_with_hdiutil(&self.image, &self.mount_point, visibility, access)?,
            ImageBackend::DiskutilImage => attach_with_diskutil(&self.image, &self.mount_point, visibility, access)?,
        };
        self.device = Some(whole_disk(&attach_output)?);
        if visibility == Visibility::Browsable {
            let indexing_disabled = run(
                "/usr/bin/mdutil",
                [OsStr::new("-i"), OsStr::new("off"), self.mount_point.as_os_str()],
            );
            if let Err(error) = indexing_disabled {
                if let Err(detach_error) = self.detach() {
                    eprintln!(
                        "image at {} was not detached: {detach_error}",
                        self.mount_point.display()
                    );
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn mount_point(&self) -> &Path {
        &self.mount_point
    }

    pub fn workspace(&self) -> &Path {
        self.workspace.path()
    }

    pub fn detach(&mut self) -> io::Result<()> {
        let Some(device) = self.device.take() else {
            return Ok(());
        };
        let deadline = Instant::now() + test_timeout();
        let mut delay = FIRST_DETACH_DELAY;
        loop {
            match self.detach_once(&device, false) {
                Ok(()) => return Ok(()),
                Err(error) if Instant::now() + delay > deadline => {
                    eprintln!("detaching {device} gracefully failed ({error}); forcing");
                    return self.detach_once(&device, true);
                }
                Err(_) => {
                    thread::sleep(delay);
                    delay *= 2;
                }
            }
        }
    }

    fn detach_once(&self, device: &str, force: bool) -> io::Result<()> {
        let output = match (self.backend, force) {
            (ImageBackend::Hdiutil, false) => run("/usr/bin/hdiutil", ["detach", device]),
            (ImageBackend::Hdiutil, true) => run("/usr/bin/hdiutil", ["detach", "-force", device]),
            (ImageBackend::DiskutilImage, false) => run("/usr/sbin/diskutil", ["eject", device]),
            (ImageBackend::DiskutilImage, true) => {
                run("/usr/sbin/diskutil", ["unmountDisk", "force", device])?;
                run("/usr/sbin/diskutil", ["eject", device])
            }
        };
        output.map(drop)
    }
}

impl Drop for ImageVolume {
    fn drop(&mut self) {
        if let Err(error) = self.detach() {
            eprintln!("image at {} was not detached: {error}", self.mount_point.display());
        }
    }
}

fn create_with_hdiutil(image: &Path, format: VolumeFormat) -> io::Result<()> {
    let file_system = match format {
        VolumeFormat::Apfs => "APFS",
        VolumeFormat::CaseSensitiveApfs => "Case-sensitive APFS",
    };
    run(
        "/usr/bin/hdiutil",
        [
            OsStr::new("create"),
            OsStr::new("-size"),
            OsStr::new(IMAGE_SIZE),
            OsStr::new("-fs"),
            OsStr::new(file_system),
            OsStr::new("-volname"),
            OsStr::new(VOLUME_NAME),
            OsStr::new("-type"),
            OsStr::new("UDIF"),
            image.as_os_str(),
        ],
    )
    .map(drop)
}

fn attach_with_hdiutil(image: &Path, mount_point: &Path, visibility: Visibility, access: Access) -> io::Result<String> {
    let mut arguments = vec![OsStr::new("attach"), OsStr::new("-owners"), OsStr::new("on")];
    if visibility == Visibility::Hidden {
        arguments.push(OsStr::new("-nobrowse"));
    }
    if access == Access::ReadOnly {
        arguments.push(OsStr::new("-readonly"));
    }
    arguments.extend([
        OsStr::new("-noverify"),
        OsStr::new("-noautofsck"),
        OsStr::new("-mountpoint"),
        mount_point.as_os_str(),
        image.as_os_str(),
    ]);
    run("/usr/bin/hdiutil", arguments)
}

fn create_with_diskutil(image: &Path, format: VolumeFormat) -> io::Result<()> {
    run(
        "/usr/sbin/diskutil",
        [
            OsStr::new("image"),
            OsStr::new("create"),
            OsStr::new("blank"),
            OsStr::new("--fs"),
            OsStr::new("APFS"),
            OsStr::new("--size"),
            OsStr::new(IMAGE_SIZE),
            OsStr::new("--volumeName"),
            OsStr::new(VOLUME_NAME),
            image.as_os_str(),
        ],
    )?;
    if format == VolumeFormat::CaseSensitiveApfs {
        reformat_case_sensitive(image)?;
    }
    Ok(())
}

fn reformat_case_sensitive(image: &Path) -> io::Result<()> {
    let attach_output = run(
        "/usr/sbin/diskutil",
        [
            OsStr::new("image"),
            OsStr::new("attach"),
            OsStr::new("--noMount"),
            image.as_os_str(),
        ],
    )?;
    let whole = whole_disk(&attach_output)?;
    let reformatted = device_with_content(&attach_output, "Apple_APFS_Container").and_then(|container| {
        let volume = device_with_content(&attach_output, "Apple_APFS_Volume")?;
        run("/usr/sbin/diskutil", ["apfs", "deleteVolume", volume.as_str()])?;
        run(
            "/usr/sbin/diskutil",
            [
                "apfs",
                "addVolume",
                container.as_str(),
                "Case-sensitive APFS",
                VOLUME_NAME,
                "-nomount",
            ],
        )
        .map(drop)
    });
    let ejected = run("/usr/sbin/diskutil", ["eject", whole.as_str()]).map(drop);
    reformatted.and(ejected)
}

fn attach_with_diskutil(
    image: &Path,
    mount_point: &Path,
    visibility: Visibility,
    access: Access,
) -> io::Result<String> {
    let mut arguments = vec![OsStr::new("image"), OsStr::new("attach")];
    if visibility == Visibility::Hidden {
        arguments.push(OsStr::new("--nobrowse"));
    }
    if access == Access::ReadOnly {
        arguments.push(OsStr::new("--readOnly"));
    }
    arguments.extend([
        OsStr::new("--mountPoint"),
        mount_point.as_os_str(),
        OsStr::new("--mountOptions"),
        OsStr::new("noowners"),
        image.as_os_str(),
    ]);
    run("/usr/sbin/diskutil", arguments)
}

fn whole_disk(attach_output: &str) -> io::Result<String> {
    attach_output
        .lines()
        .find_map(|line| {
            line.split_whitespace()
                .next()
                .filter(|device| device.starts_with("/dev/disk"))
        })
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "attach printed no device"))
}

fn device_with_content(attach_output: &str, content: &str) -> io::Result<String> {
    attach_output
        .lines()
        .find_map(|line| {
            let mut columns = line.split_whitespace();
            let device = columns.next()?;
            (columns.next()? == content).then(|| device.to_owned())
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("attach printed no {content} device"),
            )
        })
}
