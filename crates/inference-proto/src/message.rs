use std::ffi::CStr;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::time::Duration;

use domovoi_xpc_sys::XpcDictionary;

use crate::decode_error::DecodeError;
use crate::error_code::ErrorCode;
use crate::generate_request::GenerateRequest;
use crate::generation_stats::GenerationStats;
use crate::message_fields::{
    absent_or_invalid, f64_field, fd_field, i32_field, named_field, optional_text_field, text_field, u32_field,
    u64_field,
};
use crate::model_class::ModelClass;
use crate::sampling::Sampling;
use crate::stop_reason::StopReason;

pub const PROTOCOL_VERSION: u64 = 1;

const VERSION: &CStr = c"version";
const KIND: &CStr = c"kind";
const PID: &CStr = c"pid";
const WEIGHTS: &CStr = c"weights";
const CLASS: &CStr = c"class";
const CONTEXT_LENGTH: &CStr = c"context_length";
const PROMPT: &CStr = c"prompt";
const GRAMMAR: &CStr = c"grammar";
const TEMPERATURE: &CStr = c"temperature";
const TOP_K: &CStr = c"top_k";
const TOP_P: &CStr = c"top_p";
const SEED: &CStr = c"seed";
const MAX_TOKENS: &CStr = c"max_tokens";
const TEXT: &CStr = c"text";
const PROMPT_TOKENS: &CStr = c"prompt_tokens";
const GENERATED_TOKENS: &CStr = c"generated_tokens";
const PROMPT_NANOSECONDS: &CStr = c"prompt_nanoseconds";
const GENERATION_NANOSECONDS: &CStr = c"generation_nanoseconds";
const STOP: &CStr = c"stop";
const CODE: &CStr = c"code";
const DESCRIPTION: &CStr = c"description";
const COUNT: &CStr = c"count";

const HANDSHAKE: &CStr = c"handshake";
const LOAD: &CStr = c"load";
const UNLOAD: &CStr = c"unload";
const GENERATE: &CStr = c"generate";
const TOKEN: &CStr = c"token";
const DONE: &CStr = c"done";
const CANCEL: &CStr = c"cancel";
const READY: &CStr = c"ready";
const ERROR: &CStr = c"error";
const COUNT_TOKENS: &CStr = c"count_tokens";
const TOKEN_COUNT: &CStr = c"token_count";

#[derive(Debug)]
pub enum Message {
    Handshake {
        pid: i32,
    },
    Load {
        weights: OwnedFd,
        class: ModelClass,
        context_length: u32,
    },
    Unload,
    Generate(GenerateRequest),
    Token {
        text: String,
    },
    Done(GenerationStats),
    Cancel,
    Ready,
    Error {
        code: ErrorCode,
        description: String,
    },
    CountTokens {
        text: String,
    },
    TokenCount {
        count: u32,
    },
}

impl Message {
    pub fn kind(&self) -> &'static CStr {
        match self {
            Self::Handshake { .. } => HANDSHAKE,
            Self::Load { .. } => LOAD,
            Self::Unload => UNLOAD,
            Self::Generate(_) => GENERATE,
            Self::Token { .. } => TOKEN,
            Self::Done(_) => DONE,
            Self::Cancel => CANCEL,
            Self::Ready => READY,
            Self::Error { .. } => ERROR,
            Self::CountTokens { .. } => COUNT_TOKENS,
            Self::TokenCount { .. } => TOKEN_COUNT,
        }
    }

    pub fn encode(&self) -> io::Result<XpcDictionary> {
        let mut dictionary = XpcDictionary::new();
        self.encode_into(&mut dictionary)?;
        Ok(dictionary)
    }

    pub fn encode_into(&self, dictionary: &mut XpcDictionary) -> io::Result<()> {
        dictionary.set_u64(VERSION, PROTOCOL_VERSION);
        dictionary.set_string(KIND, self.kind());
        match self {
            Self::Handshake { pid } => dictionary.set_i64(PID, i64::from(*pid)),
            Self::Load {
                weights,
                class,
                context_length,
            } => {
                dictionary.set_fd(WEIGHTS, weights.as_fd())?;
                dictionary.set_string(CLASS, class.name());
                dictionary.set_u64(CONTEXT_LENGTH, u64::from(*context_length));
            }
            Self::Generate(request) => encode_generate(dictionary, request),
            Self::Token { text } | Self::CountTokens { text } => dictionary.set_bytes(TEXT, text.as_bytes()),
            Self::Done(stats) => encode_done(dictionary, stats),
            Self::Error { code, description } => {
                dictionary.set_string(CODE, code.name());
                dictionary.set_bytes(DESCRIPTION, description.as_bytes());
            }
            Self::TokenCount { count } => dictionary.set_u64(COUNT, u64::from(*count)),
            Self::Unload | Self::Cancel | Self::Ready => {}
        }
        Ok(())
    }

    pub fn decode(dictionary: &XpcDictionary) -> Result<Self, DecodeError> {
        match dictionary.u64(VERSION) {
            None => return Err(DecodeError::MissingVersion),
            Some(PROTOCOL_VERSION) => {}
            Some(other) => return Err(DecodeError::UnsupportedVersion(other)),
        }
        let kind = dictionary
            .string(KIND)
            .ok_or_else(|| absent_or_invalid(dictionary, KIND))?;
        match kind {
            kind if kind == HANDSHAKE => Ok(Self::Handshake {
                pid: i32_field(dictionary, PID)?,
            }),
            kind if kind == LOAD => Ok(Self::Load {
                weights: fd_field(dictionary, WEIGHTS)?,
                class: named_field(dictionary, CLASS, ModelClass::from_name)?,
                context_length: u32_field(dictionary, CONTEXT_LENGTH)?,
            }),
            kind if kind == UNLOAD => Ok(Self::Unload),
            kind if kind == GENERATE => decode_generate(dictionary).map(Self::Generate),
            kind if kind == TOKEN => Ok(Self::Token {
                text: text_field(dictionary, TEXT)?,
            }),
            kind if kind == DONE => decode_done(dictionary).map(Self::Done),
            kind if kind == CANCEL => Ok(Self::Cancel),
            kind if kind == READY => Ok(Self::Ready),
            kind if kind == ERROR => Ok(Self::Error {
                code: named_field(dictionary, CODE, ErrorCode::from_name)?,
                description: text_field(dictionary, DESCRIPTION)?,
            }),
            kind if kind == COUNT_TOKENS => Ok(Self::CountTokens {
                text: text_field(dictionary, TEXT)?,
            }),
            kind if kind == TOKEN_COUNT => Ok(Self::TokenCount {
                count: u32_field(dictionary, COUNT)?,
            }),
            unknown => Err(DecodeError::UnknownKind(unknown.to_string_lossy().into_owned())),
        }
    }
}

fn encode_generate(dictionary: &mut XpcDictionary, request: &GenerateRequest) {
    dictionary.set_bytes(PROMPT, request.prompt.as_bytes());
    if let Some(grammar) = &request.grammar {
        dictionary.set_bytes(GRAMMAR, grammar.as_bytes());
    }
    dictionary.set_f64(TEMPERATURE, request.sampling.temperature);
    dictionary.set_u64(TOP_K, u64::from(request.sampling.top_k));
    dictionary.set_f64(TOP_P, request.sampling.top_p);
    dictionary.set_u64(SEED, u64::from(request.sampling.seed));
    dictionary.set_u64(MAX_TOKENS, u64::from(request.max_tokens));
}

fn decode_generate(dictionary: &XpcDictionary) -> Result<GenerateRequest, DecodeError> {
    Ok(GenerateRequest {
        prompt: text_field(dictionary, PROMPT)?,
        grammar: optional_text_field(dictionary, GRAMMAR)?,
        sampling: Sampling {
            temperature: f64_field(dictionary, TEMPERATURE)?,
            top_k: u32_field(dictionary, TOP_K)?,
            top_p: f64_field(dictionary, TOP_P)?,
            seed: u32_field(dictionary, SEED)?,
        },
        max_tokens: u32_field(dictionary, MAX_TOKENS)?,
    })
}

fn encode_done(dictionary: &mut XpcDictionary, stats: &GenerationStats) {
    dictionary.set_u64(PROMPT_TOKENS, u64::from(stats.prompt_tokens));
    dictionary.set_u64(GENERATED_TOKENS, u64::from(stats.generated_tokens));
    dictionary.set_u64(PROMPT_NANOSECONDS, nanoseconds(stats.prompt_duration));
    dictionary.set_u64(GENERATION_NANOSECONDS, nanoseconds(stats.generation_duration));
    dictionary.set_string(STOP, stats.stop.name());
}

fn decode_done(dictionary: &XpcDictionary) -> Result<GenerationStats, DecodeError> {
    Ok(GenerationStats {
        prompt_tokens: u32_field(dictionary, PROMPT_TOKENS)?,
        generated_tokens: u32_field(dictionary, GENERATED_TOKENS)?,
        prompt_duration: Duration::from_nanos(u64_field(dictionary, PROMPT_NANOSECONDS)?),
        generation_duration: Duration::from_nanos(u64_field(dictionary, GENERATION_NANOSECONDS)?),
        stop: named_field(dictionary, STOP, StopReason::from_name)?,
    })
}

fn nanoseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).expect("a generation lasts less than five centuries")
}
