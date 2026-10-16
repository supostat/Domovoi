mod metal_resources;
mod nightly_model;
mod test_host;

use std::time::{Duration, Instant};

use domovoi_inference::{CancelFlag, DeviceProfile, InferenceBackend, LlamaBackend};
use domovoi_inference_proto::{GenerateRequest, Message, ModelClass, Sampling};
use test_host::{ServiceClient, TestHost, scenario};

const PROMPT: &str = "Write a long story about a lighthouse keeper who counts the ships passing at night.";
const WARM_UP_TOKENS: u32 = 16;
const MEASURED_TOKENS: u32 = 128;
const MINIMUM_INTERVALS: usize = 32;
const TOLERANCE: f64 = 0.05;
const REPORT_PREFIX: &str = "tpot_nanoseconds=";

fn request(max_tokens: u32) -> GenerateRequest {
    GenerateRequest {
        prompt: PROMPT.to_owned(),
        grammar: None,
        sampling: Sampling {
            temperature: 0.0,
            top_k: 0,
            top_p: 1.0,
            seed: 0,
        },
        max_tokens,
    }
}

fn median_interval(arrivals: &[Instant]) -> Duration {
    let mut intervals: Vec<Duration> = arrivals
        .windows(2)
        .map(|pair| pair[1].duration_since(pair[0]))
        .collect();
    assert!(
        intervals.len() >= MINIMUM_INTERVALS,
        "only {} token intervals were observed",
        intervals.len()
    );
    intervals.sort_unstable();
    intervals[intervals.len() / 2]
}

fn in_process_time_per_token() -> Duration {
    let mut backend = LlamaBackend::new(DeviceProfile::Gpu);
    backend
        .load(
            nightly_model::compact_model(),
            ModelClass::Compact,
            nightly_model::COMPACT_CONTEXT_LENGTH,
            &CancelFlag::default(),
        )
        .expect("the compact model loads on Metal");
    backend
        .generate(&request(WARM_UP_TOKENS), &CancelFlag::default(), &mut |_| {})
        .expect("the warm-up generation succeeds");
    let mut arrivals = Vec::new();
    backend
        .generate(&request(MEASURED_TOKENS), &CancelFlag::default(), &mut |_| {
            arrivals.push(Instant::now());
        })
        .expect("the measured generation succeeds");
    median_interval(&arrivals)
}

#[test]
#[ignore = "compares the compact catalog model in and out of the service on the nightly self-hosted machine"]
fn time_per_token_through_the_service_matches_in_process_generation() {
    metal_resources::place_metallibs_beside_the_test_binary();
    let in_process = in_process_time_per_token();
    let host = TestHost::build(&[("DOMOVOI_INFERENCE_BACKEND", "llama")]);
    let output = host.run("tpot", &[]);
    let reported = output
        .lines()
        .find_map(|line| line.split_once(REPORT_PREFIX).map(|(_, value)| value))
        .unwrap_or_else(|| panic!("the scenario reports its time per token\n{output}"));
    let through_service = Duration::from_nanos(reported.parse().expect("the time per token is a number"));
    let difference = (through_service.as_secs_f64() - in_process.as_secs_f64()).abs() / in_process.as_secs_f64();
    eprintln!("time per token is {through_service:?} through the service and {in_process:?} in process");
    assert!(
        difference <= TOLERANCE,
        "the service differs from in-process generation by {difference:.3}"
    );
}

#[test]
#[ignore = "runs inside the temporary TestHost.app"]
fn host_scenario() {
    let (name, service) = scenario();
    assert_eq!(name, "tpot", "unknown scenario {name}");
    let client = ServiceClient::connect(&service);
    let loaded = client.request(&Message::Load {
        weights: nightly_model::compact_model(),
        class: ModelClass::Compact,
        context_length: nightly_model::COMPACT_CONTEXT_LENGTH,
    });
    assert!(matches!(loaded, Ok(Message::Ready)), "{loaded:?}");
    let warmed = client.request(&Message::Generate(request(WARM_UP_TOKENS)));
    assert!(matches!(warmed, Ok(Message::Done(_))), "{warmed:?}");
    client.arrivals_until_quiet();
    let measured = client.request(&Message::Generate(request(MEASURED_TOKENS)));
    assert!(matches!(measured, Ok(Message::Done(_))), "{measured:?}");
    let time_per_token = median_interval(&client.arrivals_until_quiet());
    println!("{REPORT_PREFIX}{}", time_per_token.as_nanos());
    println!("scenario {name} passed");
}
