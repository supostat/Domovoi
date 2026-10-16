use std::io;
use std::num::NonZeroUsize;

use domovoi_apfs_sys::set_thread_disk_policy_utility;

const THREAD_NAME: &str = "domovoi-hash";

#[derive(Debug)]
pub struct HashingPool {
    pool: rayon::ThreadPool,
}

impl HashingPool {
    pub fn new(workers: Option<NonZeroUsize>) -> io::Result<Self> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers.map_or(0, NonZeroUsize::get))
            .thread_name(|index| format!("{THREAD_NAME}-{index}"))
            .start_handler(|_| {
                set_thread_disk_policy_utility().expect("utility disk policy applies to hashing threads");
            })
            .build()
            .map_err(io::Error::other)?;
        Ok(Self { pool })
    }

    pub fn install<R: Send>(&self, operation: impl FnOnce() -> R + Send) -> R {
        self.pool.install(operation)
    }
}
