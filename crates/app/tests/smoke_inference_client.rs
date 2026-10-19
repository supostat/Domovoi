use std::env;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_app::{InferenceClient, ServiceAddress};
use domovoi_inference::{
    AnonymousService, BackendError, CancelFlag, FakeBackend, InferenceBackend, PRESSURE_IDLE_GRACE, PressureLevel,
    ServiceSettings, serve_bundled,
};
use domovoi_inference_proto::{ErrorCode, GenerateRequest, ModelClass, Sampling, StopReason, Timeouts};
use domovoi_testkit::{TempTree, test_timeout};

const SERVICE_MODE: &str = "DOMOVOI_APP_TEST_SERVICE";
const HOST_MODE: &str = "DOMOVOI_APP_TEST_HOST";
const STALL_VARIABLE: &str = "DOMOVOI_XPC_STALL_TIMEOUT_MS";
const STALL_MILLISECONDS: &str = "2000";
const SERVICE_IDENTIFIER: &str = "com.domovoi.app.tests.inference";
const HOST_IDENTIFIER: &str = "com.domovoi.app.tests.host";
const HOST_EXECUTABLE: &str = "TestHost";
const SERVICE_EXECUTABLE: &str = "domovoi-inference";
const PLUTIL: &str = "/usr/bin/plutil";
const CODESIGN: &str = "/usr/bin/codesign";
const WEIGHTS: &[u8] = b"GGUF test weights";
const CONTEXT_LENGTH: u32 = 512;
const WAIT_STEP: Duration = Duration::from_millis(20);
const RUN_ALLOWANCE: u32 = 6;
const CANCEL_AFTER: Duration = Duration::from_millis(150);
const GRACE_MARGIN: Duration = Duration::from_millis(500);
const LOG_POLL: Duration = Duration::from_millis(250);
const CLIENT_SUBSYSTEM: &str = "com.domovoi.app";

fn main() -> ExitCode {
    if env::var_os(SERVICE_MODE).is_some() {
        let settings =
            ServiceSettings::from_process(["--backend".to_owned(), "fake".to_owned()], |name| env::var(name).ok())
                .expect("the fake service settings parse");
        match serve_bundled(settings) {
            Ok(never) => match never {},
            Err(error) => panic!("the service did not start: {error}"),
        }
    }
    if env::var_os(HOST_MODE).is_some() {
        watchdog_scenario();
        println!("scenario watchdog passed");
        return ExitCode::SUCCESS;
    }
    protocol_over_an_anonymous_endpoint();
    println!("protocol over an anonymous endpoint passed");
    memory_pressure_over_an_anonymous_endpoint();
    println!("memory pressure over an anonymous endpoint passed");
    let host = TestHost::build();
    let output = host.run();
    assert!(output.contains("scenario watchdog passed"), "{output}");
    println!("watchdog over the bundled service passed");
    ExitCode::SUCCESS
}

fn weights(workspace: &TempTree) -> OwnedFd {
    let path = workspace.path().join("weights.gguf");
    fs::write(&path, WEIGHTS).expect("the weights are written");
    File::open(path).expect("the weights open").into()
}

fn script(lines: &[&str]) -> GenerateRequest {
    GenerateRequest {
        prompt: lines.join("\n"),
        grammar: None,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
        max_tokens: 64,
    }
}

fn generate(
    client: &mut InferenceClient,
    lines: &[&str],
    cancel: &CancelFlag,
) -> (Vec<String>, Result<StopReason, BackendError>) {
    let mut tokens = Vec::new();
    let outcome = client
        .generate(&script(lines), cancel, &mut |text| tokens.push(text.to_owned()))
        .map(|stats| stats.stop);
    (tokens, outcome)
}

fn protocol_over_an_anonymous_endpoint() {
    let service = AnonymousService::start(Box::new(FakeBackend::default()), None);
    let workspace = TempTree::new().expect("a temp tree is created");
    let timeouts = Timeouts::from_environment().expect("the timeouts parse");
    let mut client =
        InferenceClient::connect(ServiceAddress::Endpoint(service.endpoint()), timeouts).expect("the client connects");
    assert_eq!(
        client.service_pid(),
        i32::try_from(std::process::id()).expect("a pid fits"),
        "the in-process service answers the handshake with its pid"
    );
    client
        .load(
            weights(&workspace),
            ModelClass::Compact,
            CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the weights load by descriptor");
    assert!(client.is_loaded());
    let (tokens, stop) = generate(&mut client, &["token Привет", "token , мир"], &CancelFlag::default());
    assert_eq!(tokens, ["Привет", ", мир"]);
    assert_eq!(stop, Ok(StopReason::EndOfGeneration));
    let cancel = CancelFlag::default();
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(CANCEL_AFTER);
            cancel.raise();
        });
        let (tokens, stop) = generate(&mut client, &["token до", "delay 5000", "token после"], &cancel);
        assert_eq!(tokens, ["до"]);
        assert_eq!(stop, Ok(StopReason::Cancelled));
    });
    assert_eq!(
        client.count_tokens("Привет"),
        Ok(u32::try_from("Привет".len()).expect("short"))
    );
}

fn client_log_shows(text: &str) -> bool {
    let predicate = format!(
        "processID == {} AND subsystem == \"{CLIENT_SUBSYSTEM}\" AND eventMessage CONTAINS \"{text}\"",
        std::process::id()
    );
    let output = Command::new("/usr/bin/log")
        .args(["show", "--last", "1m", "--style", "compact", "--predicate", &predicate])
        .output()
        .expect("log show runs");
    assert!(output.status.success(), "log show failed");
    String::from_utf8_lossy(&output.stdout).contains(text)
}

fn memory_pressure_over_an_anonymous_endpoint() {
    let service = AnonymousService::start(Box::new(FakeBackend::default()), None);
    let pressure = service.service();
    let workspace = TempTree::new().expect("a temp tree is created");
    let timeouts = Timeouts::from_environment().expect("the timeouts parse");
    let mut client =
        InferenceClient::connect(ServiceAddress::Endpoint(service.endpoint()), timeouts).expect("the client connects");
    client
        .load(
            weights(&workspace),
            ModelClass::Compact,
            CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the weights load by descriptor");

    pressure.report_pressure(PressureLevel::Warning);
    thread::sleep(PRESSURE_IDLE_GRACE + GRACE_MARGIN);
    let (tokens, stop) = generate(&mut client, &["token снова"], &CancelFlag::default());
    assert_eq!(
        tokens,
        ["снова"],
        "the client loads the weights again after the idle unload"
    );
    assert_eq!(stop, Ok(StopReason::EndOfGeneration));

    pressure.report_pressure(PressureLevel::Critical);
    pressure.report_pressure(PressureLevel::Warning);
    assert_eq!(
        client.count_tokens("Привет"),
        Ok(u32::try_from("Привет".len()).expect("short")),
        "counting tokens also restores the unloaded weights"
    );

    pressure.report_pressure(PressureLevel::Critical);
    let (tokens, stop) = generate(&mut client, &["token поздно"], &CancelFlag::default());
    assert!(tokens.is_empty());
    assert_eq!(
        stop.map_err(|error| error.code),
        Err(ErrorCode::OutOfMemory),
        "under critical pressure the reload is refused as out of memory"
    );
    assert!(!client.is_loaded(), "a refused reload leaves the client unloaded");

    let deadline = Instant::now() + test_timeout();
    while !client_log_shows("the inference service answered OutOfMemory: memory pressure is critical") {
        assert!(Instant::now() < deadline, "the refusal never reached the unified log");
        thread::sleep(LOG_POLL);
    }
}

fn watchdog_scenario() {
    let workspace = TempTree::new().expect("a temp tree is created");
    let timeouts = Timeouts::from_environment().expect("the timeouts parse");
    assert!(
        timeouts.stall <= test_timeout(),
        "the watchdog fires within the test timeout"
    );
    let name = CString::new(env::var(HOST_MODE).expect("the service is named")).expect("the name has no NUL");
    let mut client = InferenceClient::connect(ServiceAddress::Bundled(name), timeouts).expect("the client connects");
    let hung_pid = client.service_pid();
    assert!(hung_pid > 0 && hung_pid != i32::try_from(std::process::id()).expect("a pid fits"));
    client
        .load(
            weights(&workspace),
            ModelClass::Compact,
            CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the weights load by descriptor");
    let started = Instant::now();
    let (tokens, stop) = generate(&mut client, &["token перед", "hang"], &CancelFlag::default());
    assert_eq!(tokens, ["перед"]);
    assert_eq!(
        stop.map_err(|error| error.code),
        Err(ErrorCode::Stalled),
        "a hung generation ends as stalled"
    );
    assert!(
        started.elapsed() >= timeouts.stall,
        "the watchdog waited for the stall timeout"
    );
    assert_eq!(
        unsafe { libc::kill(hung_pid, 0) },
        -1,
        "the watchdog killed the hung service"
    );
    assert_ne!(
        client.service_pid(),
        hung_pid,
        "the client reconnected to a new service process"
    );
    assert!(client.is_loaded(), "the weights were loaded again after the restart");
    let (tokens, stop) = generate(&mut client, &["token снова"], &CancelFlag::default());
    assert_eq!(tokens, ["снова"]);
    assert_eq!(stop, Ok(StopReason::EndOfGeneration));
}

struct TestHost {
    _workspace: TempTree,
    application: PathBuf,
}

impl TestHost {
    fn build() -> Self {
        let workspace = TempTree::new().expect("a temp tree is created");
        let application = workspace.path().join(format!("{HOST_EXECUTABLE}.app"));
        let contents = application.join("Contents");
        let service = contents.join("XPCServices").join(format!("{SERVICE_IDENTIFIER}.xpc"));
        fs::create_dir_all(contents.join("MacOS")).expect("the host bundle is created");
        fs::create_dir_all(service.join("Contents/MacOS")).expect("the service bundle is created");
        let this = env::current_exe().expect("the test binary is known");
        fs::copy(&this, contents.join("MacOS").join(HOST_EXECUTABLE)).expect("the host executable is copied");
        fs::copy(&this, service.join("Contents/MacOS").join(SERVICE_EXECUTABLE))
            .expect("the service executable is copied");
        fs::write(contents.join("Info.plist"), host_info()).expect("the host Info.plist is written");
        let service_info = service.join("Contents/Info.plist");
        fs::copy(inference_file("bundle/Info.plist"), &service_info).expect("the service Info.plist is copied");
        plutil(
            &["-replace", "CFBundleIdentifier", "-string", SERVICE_IDENTIFIER],
            &service_info,
        );
        plutil(&["-remove", "DOMOVOITeamIdentifier"], &service_info);
        plutil(
            &[
                "-insert",
                "XPCService.EnvironmentVariables",
                "-json",
                &format!("{{\"{SERVICE_MODE}\":\"1\",\"GGML_METAL_DEVICES\":\"0\"}}"),
            ],
            &service_info,
        );
        sign(&service, Some(&inference_file("bundle/domovoi-inference.entitlements")));
        sign(&application, None);
        Self {
            _workspace: workspace,
            application,
        }
    }

    fn run(&self) -> String {
        let mut child = Command::new(self.application.join("Contents/MacOS").join(HOST_EXECUTABLE))
            .env(HOST_MODE, SERVICE_IDENTIFIER)
            .env(STALL_VARIABLE, STALL_MILLISECONDS)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the test host starts");
        let deadline = Instant::now() + test_timeout() * RUN_ALLOWANCE;
        let status = loop {
            if let Some(status) = child.try_wait().expect("the test host is polled") {
                break status;
            }
            if Instant::now() > deadline {
                child.kill().expect("the stuck test host is killed");
                break child.wait().expect("the killed test host is reaped");
            }
            thread::sleep(WAIT_STEP);
        };
        let mut output = String::new();
        child
            .stdout
            .take()
            .expect("stdout is piped")
            .read_to_string(&mut output)
            .expect("stdout reads");
        let mut errors = String::new();
        child
            .stderr
            .take()
            .expect("stderr is piped")
            .read_to_string(&mut errors)
            .expect("stderr reads");
        assert!(
            status.success(),
            "the watchdog scenario failed with {status}\n{output}\n{errors}"
        );
        output
    }
}

fn inference_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../inference")
        .join(relative)
}

fn host_info() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>{HOST_IDENTIFIER}</string>
	<key>CFBundleExecutable</key>
	<string>{HOST_EXECUTABLE}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleName</key>
	<string>{HOST_EXECUTABLE}</string>
	<key>CFBundleVersion</key>
	<string>1</string>
</dict>
</plist>
"#
    )
}

fn plutil(arguments: &[&str], plist: &Path) {
    let status = Command::new(PLUTIL)
        .args(arguments)
        .arg(plist)
        .status()
        .expect("plutil runs");
    assert!(status.success(), "plutil {arguments:?} failed");
}

fn sign(bundle: &Path, entitlements: Option<&Path>) {
    let mut command = Command::new(CODESIGN);
    command.args(["--force", "--sign", "-"]);
    if let Some(entitlements) = entitlements {
        command.arg("--entitlements").arg(entitlements);
    }
    let output = command.arg(bundle).output().expect("codesign runs");
    assert!(
        output.status.success(),
        "codesign failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
