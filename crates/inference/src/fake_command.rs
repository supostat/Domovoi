use std::ffi::CString;
use std::time::Duration;

use domovoi_inference_proto::ErrorCode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FakeCommand {
    Token(String),
    Delay(Duration),
    Fail(ErrorCode, String),
    Hang,
    Crash,
    ProbeNetwork(String),
    Weights,
}

impl FakeCommand {
    pub fn parse(line: &str) -> Option<Self> {
        let (verb, argument) = line.split_once(' ').unwrap_or((line, ""));
        match verb {
            "token" => Some(Self::Token(argument.to_owned())),
            "delay" => argument.parse().ok().map(Duration::from_millis).map(Self::Delay),
            "fail" => {
                let (code, description) = argument.split_once(' ').unwrap_or((argument, ""));
                let code = CString::new(code).ok()?;
                ErrorCode::from_name(&code).map(|code| Self::Fail(code, description.to_owned()))
            }
            "hang" => Some(Self::Hang),
            "crash" => Some(Self::Crash),
            "probe-network" if !argument.is_empty() => Some(Self::ProbeNetwork(argument.to_owned())),
            "weights" => Some(Self::Weights),
            _ => None,
        }
    }
}
