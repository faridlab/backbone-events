use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "event_mail_error_kind", rename_all = "snake_case")]
pub enum EventMailErrorKind {
    TemplateUnresolved,
    TemplateRendererNotComposed,
    RenderFailed,
    EnqueueRefused,
    RecipientInvalid,
}

impl std::fmt::Display for EventMailErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TemplateUnresolved => write!(f, "template_unresolved"),
            Self::TemplateRendererNotComposed => write!(f, "template_renderer_not_composed"),
            Self::RenderFailed => write!(f, "render_failed"),
            Self::EnqueueRefused => write!(f, "enqueue_refused"),
            Self::RecipientInvalid => write!(f, "recipient_invalid"),
        }
    }
}

impl FromStr for EventMailErrorKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "template_unresolved" => Ok(Self::TemplateUnresolved),
            "template_renderer_not_composed" => Ok(Self::TemplateRendererNotComposed),
            "render_failed" => Ok(Self::RenderFailed),
            "enqueue_refused" => Ok(Self::EnqueueRefused),
            "recipient_invalid" => Ok(Self::RecipientInvalid),
            _ => Err(format!("Unknown EventMailErrorKind variant: {}", s)),
        }
    }
}

impl Default for EventMailErrorKind {
    fn default() -> Self {
        Self::TemplateUnresolved
    }
}
