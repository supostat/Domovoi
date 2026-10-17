use std::env;
use std::io;

use crate::signing::Signing;

const IDENTITY_VARIABLE: &str = "DOMOVOI_SIGNING_IDENTITY";
const TEAM_VARIABLE: &str = "DOMOVOI_TEAM_IDENTIFIER";
const AD_HOC_FEATURES: &str = "domovoi-xpc-sys/adhoc";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BundleProfile {
    Dev,
    Adhoc,
    Release,
}

impl BundleProfile {
    pub(crate) fn from_name(name: &str) -> io::Result<Self> {
        match name {
            "dev" => Ok(Self::Dev),
            "adhoc" => Ok(Self::Adhoc),
            "release" => Ok(Self::Release),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown bundle profile {other}: expected dev, adhoc or release"),
            )),
        }
    }

    pub(crate) fn signing(self) -> io::Result<Signing> {
        match self {
            Self::Adhoc => Ok(Signing::ad_hoc()),
            Self::Dev | Self::Release => Ok(Signing {
                identity: required(IDENTITY_VARIABLE)?,
                team: Some(required(TEAM_VARIABLE)?),
                hardened: true,
            }),
        }
    }

    pub(crate) fn cargo_features(self) -> Option<&'static str> {
        match self {
            Self::Adhoc => Some(AD_HOC_FEATURES),
            Self::Dev | Self::Release => None,
        }
    }
}

pub(crate) fn required(variable: &str) -> io::Result<String> {
    env::var(variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{variable} is not set")))
}
