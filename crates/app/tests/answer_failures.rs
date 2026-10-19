use domovoi_agent::AgentError;
use domovoi_app::{ChatEntry, ChatModel, strings};
use domovoi_grammar::ResponseError;
use domovoi_inference::BackendError;
use domovoi_inference_proto::ErrorCode;

fn shown(error: &AgentError) -> String {
    let mut chat = ChatModel::default();
    chat.record_failure(error);
    match chat.entries.as_slice() {
        [ChatEntry::Failure { text }] => text.clone(),
        other => panic!("a failure is one chat entry: {other:?}"),
    }
}

fn backend(code: ErrorCode) -> AgentError {
    AgentError::Backend(BackendError::new(code, "описание сервиса"))
}

#[test]
fn every_failure_reads_as_its_cause() {
    let table = [
        (AgentError::Cancelled, strings::ANSWER_CANCELLED),
        (backend(ErrorCode::LoadCancelled), strings::ANSWER_CANCELLED),
        (backend(ErrorCode::OutOfMemory), strings::ANSWER_LOW_MEMORY),
        (backend(ErrorCode::Stalled), strings::ANSWER_STALLED),
        (backend(ErrorCode::Unavailable), strings::SERVICE_UNAVAILABLE),
        (backend(ErrorCode::ModelNotLoaded), strings::ANSWER_FAILED),
        (backend(ErrorCode::LoadFailed), strings::ANSWER_FAILED),
        (backend(ErrorCode::InvalidGrammar), strings::ANSWER_FAILED),
        (backend(ErrorCode::Busy), strings::ANSWER_FAILED),
        (backend(ErrorCode::ProtocolMismatch), strings::ANSWER_FAILED),
        (backend(ErrorCode::Internal), strings::ANSWER_FAILED),
        (
            AgentError::InvalidResponse {
                error: ResponseError::Json("EOF while parsing".to_owned()),
                text: "{\"kind\"".to_owned(),
            },
            strings::ANSWER_OUTSIDE_SCHEMA,
        ),
        (
            AgentError::ContextOverflow {
                prompt_tokens: 9_000,
                context_length: 8_192,
            },
            strings::ANSWER_TOO_LONG,
        ),
    ];
    for (error, expected) in table {
        assert_eq!(shown(&error), expected, "{error}");
    }
}
