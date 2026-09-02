use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_kanban_state", rename_all = "snake_case")]
pub enum EventKanbanState {
    Normal,
    Done,
    Blocked,
    Cancel,
}

impl std::fmt::Display for EventKanbanState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Done => write!(f, "done"),
            Self::Blocked => write!(f, "blocked"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

impl FromStr for EventKanbanState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "normal" => Ok(Self::Normal),
            "done" => Ok(Self::Done),
            "blocked" => Ok(Self::Blocked),
            "cancel" => Ok(Self::Cancel),
            _ => Err(format!("Unknown EventKanbanState variant: {}", s)),
        }
    }
}

impl Default for EventKanbanState {
    fn default() -> Self {
        Self::Normal
    }
}
