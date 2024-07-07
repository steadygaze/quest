use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

use crate::markup;
use crate::routes::prelude::*;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(preview)
}

#[derive(Template)]
#[template(source = "<pre>{{ pre_text }}</pre>", ext = "html")]
struct PreTemplate<'a> {
    pre_text: &'a str,
}

#[derive(Debug, Deserialize)]
struct PreviewForm {
    body: String,
}

#[post("/preview")]
pub async fn preview(preview_form: web::Form<PreviewForm>) -> impl Responder {
    match markup::to_html(preview_form.body.as_str()) {
        Ok(html) => html,
        Err(err) => PreTemplate {
            pre_text: format!("{}", err).as_str(),
        }
        .to_string(),
    }
}
