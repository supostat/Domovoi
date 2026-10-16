use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process;

pub(crate) fn run_handler(handler: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(handler)).is_err() {
        process::abort();
    }
}
