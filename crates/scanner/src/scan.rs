use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::mem;
use std::num::NonZeroUsize;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SendError, TrySendError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use domovoi_apfs_sys::sys::EF_NO_XATTRS;
use domovoi_apfs_sys::{
    BulkEntry, BulkListing, EntryAttributes, ObjectKind, last_used_xattr, open_at, raise_nofile_limit,
    set_thread_disk_policy_utility, set_vfs_policies, stat_at,
};

use crate::cancel_token::CancelToken;
use crate::canonical_paths::CanonicalPaths;
use crate::node::Node;
use crate::node_state::NodeState;
use crate::progress_share::{ProgressShare, SHARE_UNIT};
use crate::scan_error::ScanError;
use crate::scan_options::ScanOptions;
use crate::scan_progress::ScanProgress;
use crate::scan_report::ScanReport;
use crate::stall_monitor::{STALL_LIMIT, StallMonitor};

const WORKER_STACK_BYTES: usize = 64 * 1024 * 1024;
const WORKER_NAME: &str = "domovoi-scan";
const MONITOR_INTERVAL: Duration = Duration::from_secs(1);
const HEARTBEAT_EVERY: usize = 256;

pub fn scan(root: &Path, options: &ScanOptions) -> Result<ScanReport, ScanError> {
    raise_nofile_limit().map_err(ScanError::Setup)?;
    set_vfs_policies().map_err(ScanError::Setup)?;
    let directory = OwnedFd::from(File::open(root).map_err(ScanError::Root)?);
    let walk = Walk {
        options,
        paths: CanonicalPaths::for_root(root).map_err(ScanError::Root)?,
        files: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        deferred: Mutex::new(Vec::new()),
        monitor: StallMonitor::default(),
        share: ProgressShare::new(options.expected_directories),
        progress_order: Mutex::new(()),
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.workers.map_or(0, NonZeroUsize::get))
        .stack_size(WORKER_STACK_BYTES)
        .thread_name(|index| format!("{WORKER_NAME}-{index}"))
        .start_handler(|_| set_thread_disk_policy_utility().expect("utility disk policy applies to scan workers"))
        .build()
        .map_err(|error| ScanError::Setup(io::Error::other(error)))?;
    let mut root_node = Node {
        name: root.as_os_str().to_owned(),
        attributes: None,
        last_used: None,
        state: NodeState::Listed,
        children: Vec::new(),
    };
    let stalled = AtomicBool::new(false);
    let outcome = thread::scope(|scope| {
        let (stop, stopped) = mpsc::channel::<()>();
        let (monitor, cancel, stalled) = (&walk.monitor, &options.cancel, &stalled);
        scope.spawn(move || watch_for_stalls(monitor, cancel, stalled, &stopped));
        let outcome = walk.run(&pool, directory.as_fd(), &mut root_node);
        drop(stop);
        outcome
    });
    match outcome {
        Err(ScanError::Cancelled) if stalled.load(Ordering::Relaxed) => {
            return Err(ScanError::Unresponsive {
                path: walk.monitor.last_path(),
            });
        }
        Err(error) => return Err(error),
        Ok(()) => {}
    }
    let files = walk.files.into_inner();
    let bytes = walk.bytes.into_inner();
    if let Some(progress) = &options.progress {
        let share = walk.share.share();
        match progress.send(ScanProgress::Walked { files, bytes, share }) {
            Ok(()) | Err(SendError(_)) => {}
        }
    }
    Ok(ScanReport {
        root: root_node,
        files,
        bytes,
    })
}

pub fn list_directory(path: &Path, options: &ScanOptions) -> Result<Vec<Node>, ScanError> {
    raise_nofile_limit().map_err(ScanError::Setup)?;
    set_vfs_policies().map_err(ScanError::Setup)?;
    let directory = OwnedFd::from(File::open(path).map_err(ScanError::Root)?);
    let entries = BulkListing::new(directory.as_fd(), options.attributes)
        .collect::<io::Result<Vec<_>>>()
        .map_err(ScanError::Root)?;
    Ok(entries
        .into_iter()
        .map(|entry| listed_node(directory.as_fd(), entry))
        .collect())
}

pub fn scan_entry(parent: &Path, entry: Node, options: &ScanOptions) -> Result<Node, ScanError> {
    let path = parent.join(&entry.name);
    match scan(&path, options) {
        Ok(report) => Ok(Node {
            state: report.root.state,
            children: report.root.children,
            ..entry
        }),
        Err(ScanError::Root(error)) => Ok(Node {
            state: NodeState::of_open_error(&error, || fs::symlink_metadata(&path).map(drop)),
            children: Vec::new(),
            ..entry
        }),
        Err(error) => Err(error),
    }
}

struct Walk<'options> {
    options: &'options ScanOptions,
    paths: CanonicalPaths,
    files: AtomicU64,
    bytes: AtomicU64,
    deferred: Mutex<Vec<DeferredFolder>>,
    monitor: StallMonitor,
    share: ProgressShare,
    progress_order: Mutex<()>,
}

struct DeferredFolder {
    position: Vec<usize>,
    share: u64,
}

impl Walk<'_> {
    fn run(&self, pool: &rayon::ThreadPool, directory: BorrowedFd<'_>, root_node: &mut Node) -> Result<(), ScanError> {
        pool.install(|| self.list(directory, Path::new(""), root_node, &[], SHARE_UNIT));
        self.stop_if_cancelled()?;
        let deferred = mem::take(&mut *self.deferred.lock().expect("deferred list is not poisoned"));
        if !deferred.is_empty() {
            let heartbeat = || self.monitor.heartbeat(Instant::now());
            if !self
                .options
                .onboarding
                .wait_until_finished(&self.options.cancel, &heartbeat)
            {
                return Err(ScanError::Cancelled);
            }
            pool.install(|| self.scan_deferred(directory, root_node, &deferred));
            self.stop_if_cancelled()?;
        }
        Ok(())
    }

    fn stop_if_cancelled(&self) -> Result<(), ScanError> {
        if self.options.cancel.is_cancelled() {
            Err(ScanError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn list(&self, directory: BorrowedFd<'_>, relative: &Path, node: &mut Node, position: &[usize], share: u64) {
        if self.options.cancel.is_cancelled() {
            return;
        }
        self.monitor.enter(relative, Instant::now());
        let entries = match self.list_entries(directory) {
            Ok(entries) => entries,
            Err(error) => {
                node.state = NodeState::of_listing_error(&error);
                self.share.complete(share);
                return;
            }
        };
        let mut children: Vec<Node> = entries.into_iter().map(|entry| listed_node(directory, entry)).collect();
        let folders = children.iter().filter(|child| child.state == NodeState::Listed).count() as u64;
        let child_share = share / folders.max(1);
        self.share.complete(share - child_share * folders);
        self.share.directory_listed();
        self.report_progress(relative, &children);
        rayon::scope(|scope| {
            for (index, child) in children.iter_mut().enumerate() {
                if child.state != NodeState::Listed {
                    continue;
                }
                let child_relative = relative.join(&child.name);
                let mut child_position = position.to_vec();
                child_position.push(index);
                if self.options.protected.contains(&self.paths.canonical(&child_relative)) {
                    child.state = NodeState::Deferred;
                    self.deferred
                        .lock()
                        .expect("deferred list is not poisoned")
                        .push(DeferredFolder {
                            position: child_position,
                            share: child_share,
                        });
                    continue;
                }
                scope.spawn(move |_| self.descend(directory, &child_relative, child, &child_position, child_share));
            }
        });
        node.children = children;
    }

    fn list_entries(&self, directory: BorrowedFd<'_>) -> io::Result<Vec<BulkEntry>> {
        let mut entries = Vec::new();
        for entry in BulkListing::new(directory, self.options.attributes) {
            entries.push(entry?);
            if entries.len() % HEARTBEAT_EVERY == 0 {
                self.monitor.heartbeat(Instant::now());
            }
        }
        Ok(entries)
    }

    fn descend(&self, parent: BorrowedFd<'_>, relative: &Path, node: &mut Node, position: &[usize], share: u64) {
        match open_directory(parent, &node.name) {
            Ok(directory) => self.list(directory.as_fd(), relative, node, position, share),
            Err(error) => {
                node.state = NodeState::of_open_error(&error, || stat_at(parent, &node.name).map(drop));
                self.share.complete(share);
            }
        }
    }

    fn scan_deferred(&self, root: BorrowedFd<'_>, root_node: &mut Node, deferred: &[DeferredFolder]) {
        for folder in deferred {
            self.reopen_and_list(root, descend_to(root_node, &folder.position), folder);
        }
    }

    fn reopen_and_list(&self, root: BorrowedFd<'_>, target: DeferredTarget<'_>, folder: &DeferredFolder) {
        let DeferredTarget { node, ancestors } = target;
        let mut relative = PathBuf::new();
        let mut parent: Option<OwnedFd> = None;
        for name in &ancestors {
            let directory = parent.as_ref().map_or(root, AsFd::as_fd);
            match open_directory(directory, name) {
                Ok(opened) => {
                    relative.push(name);
                    parent = Some(opened);
                }
                Err(error) => {
                    node.state = NodeState::of_open_error(&error, || stat_at(directory, name).map(drop));
                    self.share.complete(folder.share);
                    return;
                }
            }
        }
        relative.push(&node.name);
        node.state = NodeState::Listed;
        self.descend(
            parent.as_ref().map_or(root, AsFd::as_fd),
            &relative,
            node,
            &folder.position,
            folder.share,
        );
    }

    fn report_progress(&self, relative: &Path, children: &[Node]) {
        let (files, bytes) = children
            .iter()
            .filter_map(|child| child.attributes.as_ref())
            .filter(|attributes| attributes.kind != ObjectKind::Directory)
            .fold((0_u64, 0_u64), |(files, bytes), attributes| {
                (files + 1, bytes + attributes.alloc_size.unwrap_or(0))
            });
        let files = self.files.fetch_add(files, Ordering::Relaxed) + files;
        let bytes = self.bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if let Some(progress) = &self.options.progress {
            let ordered = self.progress_order.lock().expect("progress order is not poisoned");
            let event = ScanProgress::Walking {
                files,
                bytes,
                current: self.paths.canonical(relative),
                share: self.share.share(),
            };
            match progress.try_send(event) {
                Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
            }
            drop(ordered);
        }
    }
}

struct DeferredTarget<'tree> {
    node: &'tree mut Node,
    ancestors: Vec<std::ffi::OsString>,
}

fn watch_for_stalls(monitor: &StallMonitor, cancel: &CancelToken, stalled: &AtomicBool, stopped: &Receiver<()>) {
    loop {
        match stopped.recv_timeout(MONITOR_INTERVAL) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if monitor.is_stalled(Instant::now(), STALL_LIMIT) {
                    stalled.store(true, Ordering::Relaxed);
                    cancel.cancel();
                    return;
                }
            }
        }
    }
}

fn descend_to<'tree>(root: &'tree mut Node, position: &[usize]) -> DeferredTarget<'tree> {
    let mut ancestors = Vec::new();
    let mut node = root;
    for (depth, index) in position.iter().enumerate() {
        if depth > 0 {
            ancestors.push(node.name.clone());
        }
        node = &mut node.children[*index];
    }
    DeferredTarget { node, ancestors }
}

fn listed_node(directory: BorrowedFd<'_>, entry: BulkEntry) -> Node {
    let BulkEntry { name, attributes } = entry;
    match attributes {
        Err(error) => Node {
            name,
            attributes: None,
            last_used: None,
            state: NodeState::of_entry_error(&error),
            children: Vec::new(),
        },
        Ok(attributes) => {
            let state = NodeState::of_listed_entry(&attributes);
            let last_used = if state == NodeState::Leaf && may_carry_last_used(&attributes) {
                read_last_used(directory, &name)
            } else {
                None
            };
            Node {
                name,
                attributes: Some(attributes),
                last_used,
                state,
                children: Vec::new(),
            }
        }
    }
}

fn open_directory(parent: BorrowedFd<'_>, name: &OsStr) -> io::Result<OwnedFd> {
    open_at(parent, name, libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW, 0)
}

fn may_carry_last_used(attributes: &EntryAttributes) -> bool {
    attributes.kind == ObjectKind::File
        && attributes
            .ext_flags
            .is_none_or(|flags| flags & u64::from(EF_NO_XATTRS) == 0)
}

fn read_last_used(directory: BorrowedFd<'_>, name: &OsStr) -> Option<SystemTime> {
    let file = open_at(directory, name, libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK, 0).ok()?;
    last_used_xattr(file.as_fd()).ok().flatten()
}
