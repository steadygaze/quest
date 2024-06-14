/// Wrapper library for HTML partials.
use askama_actix::Template;

use crate::app_state::{AppConfig, ProfileRenderInfo};

#[derive(Template)]
#[template(path = "partials/success.html")]
pub struct SuccessTemplate<'a> {
    pub text: &'a str,
}

#[derive(Template)]
#[template(path = "partials/failure.html")]
pub struct FailureTemplate<'a> {
    pub text: &'a str,
}

#[derive(Template)]
#[template(path = "message_page.html")]
pub struct MessagePageTemplate<'a> {
    pub config: &'a AppConfig,
    pub current_profile: &'a Option<ProfileRenderInfo>,
    pub page_title: &'a Option<&'a str>,
    pub message: &'a str,
}
