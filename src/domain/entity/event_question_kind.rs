use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_question_kind", rename_all = "snake_case")]
pub enum EventQuestionKind {
    TextBox,
    SimpleChoice,
    Name,
    Email,
    Phone,
    CompanyName,
}

impl std::fmt::Display for EventQuestionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextBox => write!(f, "text_box"),
            Self::SimpleChoice => write!(f, "simple_choice"),
            Self::Name => write!(f, "name"),
            Self::Email => write!(f, "email"),
            Self::Phone => write!(f, "phone"),
            Self::CompanyName => write!(f, "company_name"),
        }
    }
}

impl FromStr for EventQuestionKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text_box" => Ok(Self::TextBox),
            "simple_choice" => Ok(Self::SimpleChoice),
            "name" => Ok(Self::Name),
            "email" => Ok(Self::Email),
            "phone" => Ok(Self::Phone),
            "company_name" => Ok(Self::CompanyName),
            _ => Err(format!("Unknown EventQuestionKind variant: {}", s)),
        }
    }
}

impl Default for EventQuestionKind {
    fn default() -> Self {
        Self::TextBox
    }
}
