#![allow(dead_code)]

use std::env;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use domovoi_inference_proto::{GenerateRequest, Message, ModelClass, Sampling};
use domovoi_testkit::{TempTree, test_timeout};
use domovoi_xpc_sys::XpcSession;

use crate::metal_resources::place_metallibs;

pub(crate) const SCENARIO_VARIABLE: &str = "DOMOVOI_TEST_HOST_SCENARIO";
pub(crate) const SERVICE_VARIABLE: &str = "DOMOVOI_TEST_HOST_SERVICE";
pub(crate) const SERVICE_IDENTIFIER: &str = "com.domovoi.inference.tests";
pub(crate) const WEIGHTS: &[u8] = b"GGUF test weights";
const HOST_IDENTIFIER: &str = "com.domovoi.tests.host";
const HOST_EXECUTABLE: &str = "TestHost";
const SERVICE_EXECUTABLE: &str = "domovoi-inference";
const PLUTIL: &str = "/usr/bin/plutil";
const CODESIGN: &str = "/usr/bin/codesign";
const WAIT_STEP: Duration = Duration::from_millis(20);
const QUIET: Duration = Duration::from_millis(500);
const RUN_ALLOWANCE: u32 = 6;

pub(crate) struct TestHost {
    workspace: TempTree,
    application: PathBuf,
}

impl TestHost {
    pub(crate) fn build(environment: &[(&str, &str)]) -> Self {
        let workspace = TempTree::new().expect("a temp tree is created");
        let application = workspace.path().join(format!("{HOST_EXECUTABLE}.app"));
        let contents = application.join("Contents");
        let service = contents.join("XPCServices").join(format!("{SERVICE_IDENTIFIER}.xpc"));
        fs::create_dir_all(contents.join("MacOS")).expect("the host bundle is created");
        fs::create_dir_all(service.join("Contents/MacOS")).expect("the service bundle is created");
        fs::copy(
            env::current_exe().expect("the test binary is known"),
            contents.join("MacOS").join(HOST_EXECUTABLE),
        )
        .expect("the host executable is copied");
        fs::write(contents.join("Info.plist"), host_info()).expect("the host Info.plist is written");
        fs::copy(
            env!("CARGO_BIN_EXE_domovoi-inference"),
            service.join("Contents/MacOS").join(SERVICE_EXECUTABLE),
        )
        .expect("the service executable is copied");
        place_metallibs(&service.join("Contents/Resources"));
        let service_info = service.join("Contents/Info.plist");
        fs::copy(crate_file("bundle/Info.plist"), &service_info).expect("the service Info.plist is copied");
        plutil(
            &["-replace", "CFBundleIdentifier", "-string", SERVICE_IDENTIFIER],
            &service_info,
        );
        plutil(&["-remove", "DOMOVOITeamIdentifier"], &service_info);
        let variables: Vec<String> = environment
            .iter()
            .map(|(name, value)| format!("\"{name}\":\"{value}\""))
            .collect();
        plutil(
            &[
                "-insert",
                "XPCService.EnvironmentVariables",
                "-json",
                &format!("{{{}}}", variables.join(",")),
            ],
            &service_info,
        );
        sign(&service, Some(&crate_file("bundle/domovoi-inference.entitlements")));
        sign(&application, None);
        Self { workspace, application }
    }

    pub(crate) fn run(&self, scenario: &str, environment: &[(&str, String)]) -> String {
        let mut child = Command::new(self.application.join("Contents/MacOS").join(HOST_EXECUTABLE))
            .args([
                "--exact",
                "host_scenario",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(SCENARIO_VARIABLE, scenario)
            .env(SERVICE_VARIABLE, SERVICE_IDENTIFIER)
            .envs(environment.iter().map(|(name, value)| (*name, value.as_str())))
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
            "scenario {scenario} failed with {status}\n{output}\n{errors}"
        );
        output
    }
}

pub(crate) struct ServiceClient {
    session: Arc<XpcSession>,
    tokens: Receiver<(Instant, String)>,
}

impl ServiceClient {
    pub(crate) fn connect(service: &str) -> Self {
        let (token_sender, tokens) = mpsc::channel();
        let name = CString::new(service).expect("the service name has no NUL");
        let session = XpcSession::connect_service(
            &name,
            move |message| {
                if let Ok(Message::Token { text }) = Message::decode(&message) {
                    let _ = token_sender.send((Instant::now(), text));
                }
            },
            |_| {},
        )
        .expect("the service session starts");
        Self {
            session: Arc::new(session),
            tokens,
        }
    }

    pub(crate) fn request(&self, message: &Message) -> Result<Message, String> {
        let reply = self
            .session
            .send_with_reply_sync(&message.encode().expect("the request encodes"))
            .map_err(|error| error.description)?;
        Message::decode(&reply).map_err(|error| error.to_string())
    }

    pub(crate) fn session(&self) -> Arc<XpcSession> {
        Arc::clone(&self.session)
    }

    pub(crate) fn tokens(&self, count: usize) -> Vec<String> {
        let deadline = Instant::now() + test_timeout();
        let mut received = Vec::new();
        while received.len() < count && Instant::now() < deadline {
            if let Ok((_, text)) = self.tokens.recv_timeout(WAIT_STEP) {
                received.push(text);
            }
        }
        received
    }

    pub(crate) fn arrivals_until_quiet(&self) -> Vec<Instant> {
        let mut arrivals = Vec::new();
        while let Ok((arrived, _)) = self.tokens.recv_timeout(QUIET) {
            arrivals.push(arrived);
        }
        arrivals
    }
}

pub(crate) fn weights(workspace: &TempTree) -> OwnedFd {
    let path = workspace.path().join("weights.gguf");
    fs::write(&path, WEIGHTS).expect("the weights are written");
    File::open(path).expect("the weights open").into()
}

pub(crate) fn load(client: &ServiceClient, workspace: &TempTree) -> Result<Message, String> {
    client.request(&Message::Load {
        weights: weights(workspace),
        class: ModelClass::Compact,
        context_length: 512,
    })
}

pub(crate) fn script(lines: &[&str]) -> Message {
    Message::Generate(GenerateRequest {
        prompt: lines.join("\n"),
        grammar: None,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
        max_tokens: 64,
    })
}

pub(crate) fn scenario() -> (String, String) {
    (
        env::var(SCENARIO_VARIABLE).expect("the scenario is named"),
        env::var(SERVICE_VARIABLE).expect("the service is named"),
    )
}

fn crate_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
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
