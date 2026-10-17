use crate::availability::Availability;
use crate::model_variant::ModelVariant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Access,
    Scanning,
    Main,
    Recommendations,
    Assistant(Availability),
    Duplicates,
    Model(ModelVariant),
    Tray,
}

impl Screen {
    pub const ALL: [Self; 14] = [
        Self::Access,
        Self::Scanning,
        Self::Main,
        Self::Recommendations,
        Self::Assistant(Availability::Ready),
        Self::Assistant(Availability::Answering),
        Self::Assistant(Availability::Loading),
        Self::Assistant(Availability::Unloaded),
        Self::Assistant(Availability::NoModel),
        Self::Assistant(Availability::Off),
        Self::Duplicates,
        Self::Model(ModelVariant::Catalog),
        Self::Model(ModelVariant::Off),
        Self::Tray,
    ];

    pub fn number(self) -> &'static str {
        match self {
            Self::Access => "01",
            Self::Scanning => "02",
            Self::Main => "03",
            Self::Recommendations => "04",
            Self::Assistant(_) => "05",
            Self::Duplicates => "06",
            Self::Model(_) => "07",
            Self::Tray => "08",
        }
    }

    pub fn variant(self) -> Option<&'static str> {
        match self {
            Self::Assistant(Availability::Answering) => Some("answering"),
            Self::Assistant(Availability::Loading) => Some("loading"),
            Self::Assistant(Availability::Unloaded) => Some("unloaded"),
            Self::Assistant(Availability::NoModel) => Some("no-model"),
            Self::Assistant(Availability::Off) | Self::Model(ModelVariant::Off) => Some("off"),
            Self::Access
            | Self::Scanning
            | Self::Main
            | Self::Recommendations
            | Self::Assistant(Availability::Ready)
            | Self::Duplicates
            | Self::Model(ModelVariant::Catalog)
            | Self::Tray => None,
        }
    }

    pub fn with_number(number: &str) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|screen| screen.number() == number)
            .collect()
    }

    pub fn primary() -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|screen| screen.variant().is_none())
            .collect()
    }
}
