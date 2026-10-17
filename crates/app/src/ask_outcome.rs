use domovoi_agent::{AgentAnswer, AgentError};

use crate::inference_client::InferenceClient;
use crate::session::Session;

#[derive(Debug)]
pub(crate) struct AskOutcome {
    pub session: Session,
    pub client: Option<InferenceClient>,
    pub answer: Result<AgentAnswer, AgentError>,
}
