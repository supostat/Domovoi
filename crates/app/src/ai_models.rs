use crate::chat_model::ChatModel;
use crate::model_screen_model::ModelScreenModel;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiModels {
    pub chat: ChatModel,
    pub model: ModelScreenModel,
}
