use serde::Deserialize;

use crate::ai_models::AiModels;
use crate::chat_model::ChatModel;
use crate::model_screen_model::ModelScreenModel;
use crate::model_variant::ModelVariant;
use crate::screen::Screen;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiFixture {
    pub chat: ChatModel,
    pub model: ModelScreenModel,
    pub model_off: ModelScreenModel,
}

impl AiFixture {
    pub fn models_for(&self, screen: Screen) -> AiModels {
        let mut chat = self.chat.clone();
        if let Screen::Assistant(availability) = screen {
            chat.availability = availability;
        }
        let model = match screen {
            Screen::Model(ModelVariant::Off) => self.model_off.clone(),
            _ => self.model.clone(),
        };
        AiModels { chat, model }
    }
}
