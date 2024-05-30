use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;
use actix_web::get;
use actix_web::post;
use actix_web::web;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web::Responder;
use anyhow::Context;
use askama_actix::Template;
use askama_actix::TemplateToResponse;

use crate::app_state::AppConfig;
use crate::app_state::AppState;
use crate::error::Error;
use crate::error::Result;
use crate::session::get_session_info;
use crate::session::SESSION_ID_COOKIE;

pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    app.service(create_new_quest_form)
        .service(create_new_quest_submit)
}

#[derive(Template)]
#[template(path = "create_quest.html")]
struct CreateQuestTemplate<'a> {
    config: &'a AppConfig,
}

#[get("/qm/new")]
async fn create_new_quest_form(
    app_state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let session_info = app_state.get_session(request).await?;

    Ok(CreateQuestTemplate {
        config: &app_state.config,
    }
    .to_response())
}

#[post("/qm/new")]
async fn create_new_quest_submit() -> Result<impl Responder> {
    Ok(HttpResponse::Ok().body(""))
}
