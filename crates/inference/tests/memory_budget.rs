mod metal_resources;
mod nightly_model;

use std::ffi::{CStr, OsString};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use domovoi_apfs_sys::{EntryAttributes, ObjectKind};
use domovoi_index::{Enrichment, Index, IndexRoot};
use domovoi_inference::{CancelFlag, DeviceProfile, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::{GenerateRequest, GenerationStats, ModelClass, Sampling};
use domovoi_scanner::{Node, NodeState, ScanReport};

const DIRECTORIES: u64 = 2_000;
const FILES_PER_DIRECTORY: u64 = 1_000;
const INDEX_NODES: usize = 2_000_000;
const FILE_BYTES: u64 = 4096;
const CREATED_SECONDS: u64 = 1_700_000_000;
const FIXTURE_ROOT: &str = "/Volumes/Fixture";
const PRESSURE_LEVEL: &CStr = c"kern.memorystatus_vm_pressure_level";
const CRITICAL_LEVEL: i32 = 4;
const SAMPLE_PERIOD: Duration = Duration::from_millis(50);
const SENTENCE: &str = "The folder holds project sources, build caches and old disk images. ";
const REPETITIONS: usize = 150;
const MAX_TOKENS: u32 = 64;

fn attributes(kind: ObjectKind, file_id: u64) -> EntryAttributes {
    let created = UNIX_EPOCH + Duration::from_secs(CREATED_SECONDS);
    let is_file = kind == ObjectKind::File;
    EntryAttributes {
        kind,
        flags: 0,
        file_id,
        created,
        modified: created,
        added: Some(created),
        mount_status: (!is_file).then_some(0),
        link_count: is_file.then_some(1),
        total_size: is_file.then_some(FILE_BYTES),
        alloc_size: is_file.then_some(FILE_BYTES),
        data_alloc_size: is_file.then_some(FILE_BYTES),
        private_size: None,
        link_id: Some(file_id),
        real_dev: Some(1),
        clone_id: Some(file_id),
        ext_flags: Some(0),
        clone_refcount: is_file.then_some(1),
    }
}

fn large_index() -> Index {
    let mut next_file_id = 2;
    let mut directories = Vec::new();
    for directory in 0..DIRECTORIES {
        let mut files = Vec::new();
        for file in 0..FILES_PER_DIRECTORY {
            next_file_id += 1;
            files.push(Node {
                name: OsString::from(format!("file-{file}.dat")),
                attributes: Some(attributes(ObjectKind::File, next_file_id)),
                last_used: None,
                state: NodeState::Leaf,
                children: Vec::new(),
            });
        }
        next_file_id += 1;
        directories.push(Node {
            name: OsString::from(format!("directory-{directory}")),
            attributes: Some(attributes(ObjectKind::Directory, next_file_id)),
            last_used: None,
            state: NodeState::Listed,
            children: files,
        });
    }
    let report = ScanReport {
        root: Node {
            name: OsString::from(FIXTURE_ROOT),
            attributes: Some(attributes(ObjectKind::Directory, 1)),
            last_used: None,
            state: NodeState::Listed,
            children: directories,
        },
        files: DIRECTORIES * FILES_PER_DIRECTORY,
        bytes: DIRECTORIES * FILES_PER_DIRECTORY * FILE_BYTES,
    };
    let root = IndexRoot::Plain {
        path: FIXTURE_ROOT.to_owned(),
    };
    Index::build(&report, root, &Enrichment::default())
}

fn pressure_level() -> i32 {
    let mut level: i32 = 0;
    let mut size = size_of::<i32>();
    let status = unsafe {
        libc::sysctlbyname(
            PRESSURE_LEVEL.as_ptr(),
            (&raw mut level).cast(),
            &raw mut size,
            ptr::null_mut(),
            0,
        )
    };
    assert_eq!(status, 0, "the memory pressure level reads");
    level
}

fn worst_pressure_during<T>(work: impl FnOnce() -> T) -> (T, i32) {
    let finished = AtomicBool::new(false);
    thread::scope(|scope| {
        let watcher = scope.spawn(|| {
            let mut worst = pressure_level();
            while !finished.load(Ordering::SeqCst) {
                thread::sleep(SAMPLE_PERIOD);
                worst = worst.max(pressure_level());
            }
            worst
        });
        let outcome = work();
        finished.store(true, Ordering::SeqCst);
        (outcome, watcher.join().expect("the pressure watcher finishes"))
    })
}

fn load_and_generate() -> GenerationStats {
    let mut backend = LlamaBackend::new(DeviceProfile::Gpu);
    backend
        .load(
            nightly_model::compact_model(),
            ModelClass::Compact,
            nightly_model::COMPACT_CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the compact class fits the GPU working set and loads");
    let request = GenerateRequest {
        prompt: SENTENCE.repeat(REPETITIONS),
        grammar: None,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
        max_tokens: MAX_TOKENS,
    };
    backend
        .generate(&request, &CancelFlag::default(), &mut |_| {})
        .expect("generation succeeds")
}

#[test]
#[ignore = "loads the compact catalog model beside a two-million-node index on the nightly self-hosted machine"]
fn the_compact_class_loads_beside_a_large_index_without_critical_pressure() {
    metal_resources::place_metallibs_beside_the_test_binary();
    let index = large_index();
    assert!(index.len() >= INDEX_NODES, "the index holds {} nodes", index.len());
    let (stats, worst) = worst_pressure_during(load_and_generate);
    assert!(stats.generated_tokens > 0, "{stats:?}");
    assert!(
        worst < CRITICAL_LEVEL,
        "memory pressure reached level {worst} with {} index nodes open",
        index.len()
    );
}
