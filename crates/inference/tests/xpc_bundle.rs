mod metal_resources;
mod test_host;

use std::process;
use std::thread;
use std::time::Duration;

use domovoi_inference_proto::{ErrorCode, Message, StopReason};
use domovoi_testkit::TempTree;
use test_host::{ServiceClient, TestHost, WEIGHTS, load, scenario, script};

const IDLE_UNLOAD_SECONDS: &str = "1";
const IDLE_WAIT: Duration = Duration::from_millis(1800);
const CANCEL_DELAY: Duration = Duration::from_millis(200);

#[test]
fn the_bundled_service_round_trips_recovers_from_a_crash_and_unloads_when_idle() {
    let host = TestHost::build(&[
        ("DOMOVOI_INFERENCE_BACKEND", "fake"),
        ("DOMOVOI_INFERENCE_IDLE_UNLOAD_SECS", IDLE_UNLOAD_SECONDS),
        ("GGML_METAL_DEVICES", "0"),
    ]);
    for name in ["roundtrip", "crash", "idle"] {
        let output = host.run(name, &[]);
        assert!(output.contains(&format!("scenario {name} passed")), "{output}");
    }
}

#[test]
#[ignore = "runs inside the temporary TestHost.app"]
fn host_scenario() {
    let (name, service) = scenario();
    match name.as_str() {
        "roundtrip" => round_trip(&service),
        "crash" => crash_and_reconnect(&service),
        "idle" => idle_unload(&service),
        other => panic!("unknown scenario {other}"),
    }
    println!("scenario {name} passed");
}

fn handshake(client: &ServiceClient) -> i32 {
    let own = i32::try_from(process::id()).expect("a pid fits in i32");
    match client.request(&Message::Handshake { pid: own }) {
        Ok(Message::Handshake { pid }) => {
            assert!(pid > 0 && pid != own, "the service reports its own pid {pid}");
            pid
        }
        other => panic!("expected a handshake, got {other:?}"),
    }
}

fn round_trip(service: &str) {
    let workspace = TempTree::new().expect("a temp tree is created");
    let client = ServiceClient::connect(service);
    handshake(&client);
    assert!(matches!(load(&client, &workspace), Ok(Message::Ready)));

    let Ok(Message::Done(stats)) = client.request(&script(&["token Привет", "token , мир", "weights"])) else {
        panic!("the scripted generation completes");
    };
    assert_eq!(stats.generated_tokens, 2);
    assert_eq!(
        client.tokens(3),
        ["Привет", ", мир", &String::from_utf8_lossy(&WEIGHTS[..4])]
    );

    assert!(matches!(
        client.request(&script(&["fail invalid_grammar broken rule"])),
        Ok(Message::Error {
            code: ErrorCode::InvalidGrammar,
            ..
        })
    ));

    let session = client.session();
    let generation = thread::spawn(move || {
        let reply = session
            .send_with_reply_sync(&script(&["hang"]).encode().expect("the request encodes"))
            .expect("the hanging generation replies");
        Message::decode(&reply).expect("the reply decodes")
    });
    thread::sleep(CANCEL_DELAY);
    assert!(matches!(client.request(&Message::Cancel), Ok(Message::Ready)));
    match generation.join().expect("the generation thread finishes") {
        Message::Done(stats) => assert_eq!(stats.stop, StopReason::Cancelled),
        other => panic!("expected a cancelled generation, got {other:?}"),
    }
}

fn crash_and_reconnect(service: &str) {
    let workspace = TempTree::new().expect("a temp tree is created");
    let first = ServiceClient::connect(service);
    let crashed_pid = handshake(&first);
    assert!(matches!(load(&first, &workspace), Ok(Message::Ready)));
    assert!(
        first.request(&script(&["crash"])).is_err(),
        "a crashed service cannot reply"
    );

    let second = ServiceClient::connect(service);
    let restarted_pid = handshake(&second);
    assert_ne!(crashed_pid, restarted_pid, "launchd starts a new service process");
    assert!(matches!(
        second.request(&script(&["token x"])),
        Ok(Message::Error {
            code: ErrorCode::ModelNotLoaded,
            ..
        })
    ));
}

fn idle_unload(service: &str) {
    let workspace = TempTree::new().expect("a temp tree is created");
    let client = ServiceClient::connect(service);
    handshake(&client);
    assert!(matches!(load(&client, &workspace), Ok(Message::Ready)));
    assert!(matches!(client.request(&script(&["token x"])), Ok(Message::Done(_))));
    thread::sleep(IDLE_WAIT);
    assert!(matches!(
        client.request(&script(&["token x"])),
        Ok(Message::Error {
            code: ErrorCode::ModelNotLoaded,
            ..
        })
    ));
}
