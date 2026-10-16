use std::collections::BTreeSet;
use std::env;
use std::io;
use std::path::{Component, Path, PathBuf};

use domovoi_apfs_sys::sys::{
    SF_APPEND, SF_FIRMLINK, SF_IMMUTABLE, SF_NOUNLINK, SF_RESTRICTED, UF_APPEND, UF_DATAVAULT, UF_IMMUTABLE,
};
use domovoi_apfs_sys::{mounted_volumes, real_dev, running_bundle_roots, user_access};
use domovoi_index::{Index, NodeId, NodeKind, Reach};

use crate::deny_reason::DenyReason;
use crate::name_match::{CaseRule, normalized};
use crate::path_pattern::PathPattern;

const PROTECTED_FLAGS: [(u32, &str); 8] = [
    (SF_RESTRICTED, "SF_RESTRICTED"),
    (SF_NOUNLINK, "SF_NOUNLINK"),
    (UF_IMMUTABLE, "UF_IMMUTABLE"),
    (SF_IMMUTABLE, "SF_IMMUTABLE"),
    (UF_APPEND, "UF_APPEND"),
    (SF_APPEND, "SF_APPEND"),
    (UF_DATAVAULT, "UF_DATAVAULT"),
    (SF_FIRMLINK, "SF_FIRMLINK"),
];
const PROTECTED_DATA: [(&str, &str); 24] = [
    ("~/Library/Keychains", "связки ключей"),
    ("~/Library/Preferences", "настройки программ"),
    ("~/Library/Containers/*/Data/Library/Preferences", "настройки программ"),
    ("~/Library/Containers/com.apple.*", "данные программ Apple"),
    ("~/Library/Group Containers/group.com.apple.*", "данные программ Apple"),
    ("~/Library/Group Containers/*.com.apple.*", "данные программ Apple"),
    ("~/Library/Mobile Documents", "iCloud Drive"),
    ("~/Library/CloudStorage", "облачные хранилища"),
    ("~/Library/Mail", "Почта"),
    ("~/Library/Messages", "Сообщения"),
    ("~/Library/Safari", "Safari"),
    ("~/Library/Cookies", "cookies"),
    ("~/Library/HomeKit", "Дом"),
    ("~/Library/Suggestions", "подсказки Siri"),
    ("~/Library/Metadata/CoreSpotlight", "индекс Spotlight"),
    ("~/Library/Accounts", "учётные записи"),
    ("~/Library/Application Support/AddressBook", "Контакты"),
    ("~/Library/Calendars", "Календарь"),
    ("**/*.photoslibrary", "медиатека Фото"),
    ("~/Music/Music", "медиатека Музыки"),
    ("/.Spotlight-V100", "служебные данные тома"),
    ("/.fseventsd", "служебные данные тома"),
    ("/.DocumentRevisions-V100", "служебные данные тома"),
    ("**/.Trashes", "служебные данные тома"),
];
const TRASH: &str = "~/.Trash";
const OWN_DATA: [&str; 2] = [
    "~/Library/Application Support/com.domovoi.app",
    "~/Library/Caches/com.domovoi.app",
];
const SYSTEM_ROOT: &str = "/";

#[derive(Debug)]
pub struct Denylist {
    case: CaseRule,
    system_real_dev: u32,
    running_bundles: BTreeSet<PathBuf>,
    read_only_mounts: Vec<PathBuf>,
    own_bundle: Option<PathBuf>,
    protected: Vec<(PathPattern, &'static str)>,
    trash: PathPattern,
    own_data: Vec<PathPattern>,
}

impl Denylist {
    pub fn new(case: CaseRule) -> io::Result<Self> {
        let fold = |pattern: &str| PathPattern::parse(pattern).folded(case);
        Ok(Self {
            case,
            system_real_dev: real_dev(Path::new(SYSTEM_ROOT))?,
            running_bundles: running_bundle_roots()?,
            read_only_mounts: read_only_mounts()?,
            own_bundle: env::current_exe()?
                .ancestors()
                .filter(|ancestor| ancestor.extension().is_some_and(|extension| extension == "app"))
                .last()
                .map(Path::to_path_buf),
            protected: PROTECTED_DATA
                .iter()
                .map(|(pattern, what)| (fold(pattern), *what))
                .collect(),
            trash: fold(TRASH),
            own_data: OWN_DATA.iter().map(|pattern| fold(pattern)).collect(),
        })
    }

    pub fn refresh(&mut self) -> io::Result<()> {
        self.running_bundles = running_bundle_roots()?;
        self.read_only_mounts = read_only_mounts()?;
        Ok(())
    }

    pub fn verdict(&self, index: &Index, node: NodeId) -> Option<DenyReason> {
        let record = index.node(node);
        if record.reach == Reach::OtherVolume {
            return Some(DenyReason::OtherVolume);
        }
        if record.reach == Reach::Firmlink {
            return Some(DenyReason::Firmlink);
        }
        if record.kind != NodeKind::Unknown && record.real_dev == self.system_real_dev {
            return Some(DenyReason::SystemVolume);
        }
        let path = index.path(node);
        if let Some(reason) = self.path_reason(&path) {
            return Some(reason);
        }
        if let Some(flag) = protected_flag_in_subtree(index, node) {
            return Some(DenyReason::ProtectedFlag(flag));
        }
        if let Some(bundle) = self
            .running_bundles
            .iter()
            .find(|bundle| bundle.starts_with(&path) || path.starts_with(bundle))
        {
            return Some(DenyReason::RunningApplication(bundle.clone()));
        }
        let on_read_only_volume = self
            .read_only_mounts
            .iter()
            .any(|mount| index.physical_path(node).starts_with(mount));
        if !on_read_only_volume && parent_is_read_only(index, node) {
            return Some(DenyReason::ParentNotWritable);
        }
        None
    }

    fn path_reason(&self, path: &Path) -> Option<DenyReason> {
        if self
            .own_bundle
            .as_ref()
            .is_some_and(|bundle| bundle.starts_with(path) || path.starts_with(bundle))
        {
            return Some(DenyReason::OwnApplication);
        }
        let components: Vec<String> = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(normalized(name, self.case)),
                _ => None,
            })
            .collect();
        let mut prefix = Vec::with_capacity(components.len());
        let has_sibling = |_: &str| false;
        for component in components {
            prefix.push(component);
            if self.trash.matches(&prefix, has_sibling) {
                return Some(DenyReason::Trash);
            }
            if self
                .own_data
                .iter()
                .any(|pattern| pattern.matches(&prefix, has_sibling))
            {
                return Some(DenyReason::OwnApplication);
            }
            if let Some((_, what)) = self
                .protected
                .iter()
                .find(|(pattern, _)| pattern.matches(&prefix, has_sibling))
            {
                return Some(DenyReason::ProtectedData(what));
            }
        }
        None
    }
}

fn parent_is_read_only(index: &Index, node: NodeId) -> bool {
    let Some(parent) = index.parent(node) else {
        return true;
    };
    let writable = libc::c_uint::try_from(libc::W_OK).expect("W_OK fits in c_uint");
    user_access(&index.physical_path(parent)).is_ok_and(|access| access & writable == 0)
}

fn read_only_mounts() -> io::Result<Vec<PathBuf>> {
    Ok(mounted_volumes()?
        .into_iter()
        .filter(|volume| volume.read_only && volume.mount_point != Path::new(SYSTEM_ROOT))
        .map(|volume| volume.mount_point)
        .collect())
}

fn protected_flag_in_subtree(index: &Index, node: NodeId) -> Option<&'static str> {
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        let flags = index.node(current).flags;
        if let Some((_, name)) = PROTECTED_FLAGS.iter().find(|(flag, _)| flags & flag != 0) {
            return Some(name);
        }
        pending.extend(index.children(current));
    }
    None
}
