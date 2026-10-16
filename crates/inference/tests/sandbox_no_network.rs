mod metal_resources;
mod test_host;

use std::env;
use std::io;
use std::net::{TcpListener, TcpStream};

use domovoi_inference_proto::Message;
use domovoi_testkit::TempTree;
use test_host::{ServiceClient, TestHost, WEIGHTS, load, scenario, script};

const PROBE_ADDRESS_VARIABLE: &str = "DOMOVOI_TEST_PROBE_ADDRESS";

#[test]
fn the_sandboxed_service_reads_passed_weights_but_cannot_open_sockets() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local listener binds");
    let address = listener.local_addr().expect("the listener has an address").to_string();
    let host = TestHost::build(&[("DOMOVOI_INFERENCE_BACKEND", "fake")]);
    let output = host.run("sandbox", &[(PROBE_ADDRESS_VARIABLE, address)]);
    assert!(output.contains("scenario sandbox passed"), "{output}");
}

#[test]
#[ignore = "runs inside the temporary TestHost.app"]
fn host_scenario() {
    let (name, service) = scenario();
    assert_eq!(name, "sandbox", "unknown scenario {name}");
    let address = env::var(PROBE_ADDRESS_VARIABLE).expect("the probe address is passed");
    TcpStream::connect(&address).expect("the unsandboxed host reaches the listener");

    let workspace = TempTree::new().expect("a temp tree is created");
    let client = ServiceClient::connect(&service);
    assert!(matches!(load(&client, &workspace), Ok(Message::Ready)));
    assert!(matches!(
        client.request(&script(&[&format!("probe-network {address}"), "weights"])),
        Ok(Message::Done(_))
    ));
    let denied = io::Error::from_raw_os_error(libc::EPERM);
    assert_eq!(
        client.tokens(2),
        [
            format!("network: {denied}"),
            String::from_utf8_lossy(&WEIGHTS[..4]).into_owned()
        ]
    );
    println!("scenario {name} passed");
}
