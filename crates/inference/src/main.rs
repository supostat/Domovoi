use std::env;
use std::ffi::CString;
use std::process::ExitCode;
use std::sync::Arc;

use domovoi_apfs_sys::set_vfs_policies;
use domovoi_inference::{InferenceService, MemoryPressureMonitor, ServiceSettings};
use domovoi_xpc_sys::{PEER_REQUIREMENT_ENFORCED, XpcListener};
use objc2_foundation::{NSBundle, NSString};

const TEAM_IDENTIFIER_KEY: &str = "DOMOVOITeamIdentifier";

fn main() -> ExitCode {
    if let Err(error) = set_vfs_policies() {
        eprintln!("domovoi-inference: VFS policies could not be set: {error}");
        return ExitCode::FAILURE;
    }
    match serve() {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("domovoi-inference: {error}");
            ExitCode::FAILURE
        }
    }
}

fn serve() -> Result<std::convert::Infallible, String> {
    let settings = ServiceSettings::from_process(env::args().skip(1), |name| env::var(name).ok())?;
    let service = Arc::new(InferenceService::start(settings.create_backend(), settings.idle_unload));
    let _monitor = MemoryPressureMonitor::start(Arc::clone(&service));
    let bundle = NSBundle::mainBundle();
    let identifier = bundle
        .bundleIdentifier()
        .ok_or("the service runs outside its bundle")?
        .to_string();
    let team_identifier = bundle
        .objectForInfoDictionaryKey(&NSString::from_str(TEAM_IDENTIFIER_KEY))
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|value| value.to_string())
        .unwrap_or_default();
    if PEER_REQUIREMENT_ENFORCED && team_identifier.is_empty() {
        return Err(format!("{TEAM_IDENTIFIER_KEY} is missing from Info.plist"));
    }
    let service_name = CString::new(identifier).map_err(|_| "the bundle identifier contains NUL")?;
    let _listener = XpcListener::new(&service_name, &team_identifier, move |session| {
        let service = Arc::clone(&service);
        session.accept(move |peer, message| service.handle(message, Arc::new(peer.clone())));
    })
    .map_err(|error| format!("the listener did not start: {error}"))?;
    dispatch2::dispatch_main()
}
