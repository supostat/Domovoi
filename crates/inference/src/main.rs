use std::env;
use std::process::ExitCode;

use domovoi_apfs_sys::set_vfs_policies;
use domovoi_inference::{ServiceSettings, serve_bundled};

fn main() -> ExitCode {
    if let Err(error) = set_vfs_policies() {
        eprintln!("domovoi-inference: VFS policies could not be set: {error}");
        return ExitCode::FAILURE;
    }
    let served = ServiceSettings::from_process(env::args().skip(1), |name| env::var(name).ok()).and_then(serve_bundled);
    match served {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("domovoi-inference: {error}");
            ExitCode::FAILURE
        }
    }
}
