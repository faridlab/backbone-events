use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_badge_format", rename_all = "snake_case")]
pub enum EventBadgeFormat {
    A4FrenchFold,
    A6,
    FourPerSheet,
}

impl std::fmt::Display for EventBadgeFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A4FrenchFold => write!(f, "a4_french_fold"),
            Self::A6 => write!(f, "a6"),
            Self::FourPerSheet => write!(f, "four_per_sheet"),
        }
    }
}

impl FromStr for EventBadgeFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "a4_french_fold" => Ok(Self::A4FrenchFold),
            "a6" => Ok(Self::A6),
            "four_per_sheet" => Ok(Self::FourPerSheet),
            _ => Err(format!("Unknown EventBadgeFormat variant: {}", s)),
        }
    }
}

impl Default for EventBadgeFormat {
    fn default() -> Self {
        Self::A4FrenchFold
    }
}
