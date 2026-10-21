use crate::ask_reply::AskReply;
use crate::inference_client::InferenceClient;
use crate::session::Session;

#[derive(Debug)]
pub struct AskOutcome {
    pub session: Session,
    pub client: Option<InferenceClient>,
    pub reply: AskReply,
}
