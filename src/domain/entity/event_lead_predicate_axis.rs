use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_lead_predicate_axis", rename_all = "snake_case")]
pub enum EventLeadPredicateAxis {
    Event,
    EventType,
    Company,
    QuestionAnswer,
}

impl std::fmt::Display for EventLeadPredicateAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Event => write!(f, "event"),
            Self::EventType => write!(f, "event_type"),
            Self::Company => write!(f, "company"),
            Self::QuestionAnswer => write!(f, "question_answer"),
        }
    }
}

impl FromStr for EventLeadPredicateAxis {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "event" => Ok(Self::Event),
            "event_type" => Ok(Self::EventType),
            "company" => Ok(Self::Company),
            "question_answer" => Ok(Self::QuestionAnswer),
            _ => Err(format!("Unknown EventLeadPredicateAxis variant: {}", s)),
        }
    }
}

impl Default for EventLeadPredicateAxis {
    fn default() -> Self {
        Self::Event
    }
}
