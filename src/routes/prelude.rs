/// Prelude module for importing common route symbols. This reduces boilerplate
/// and time to set up new routes.
pub use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
pub use anyhow::Context;
pub use askama_actix::{Template, TemplateToResponse};
pub use log::{info, trace, warn};
pub use serde::Deserialize;
pub use tokio::{join, try_join};
pub use uuid::Uuid;

pub use crate::app_state::{AppConfig, AppState, ProfileRenderInfo, SessionInfo};
pub use crate::error::{Error, Result};
pub use crate::partials::*;
pub use crate::validation;
