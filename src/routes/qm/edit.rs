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
use serde::Deserialize;

use crate::app_state::AppConfig;
use crate::app_state::AppState;
use crate::error::Error;
use crate::error::Result;
use crate::partials;
use crate::session::get_session_info;
use crate::session::SESSION_ID_COOKIE;

pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    app.service(create_new_quest_form)
        .service(create_new_quest_submit)
        .service(check_existing_slug)
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
    let (session_info, user_id) = app_state.get_session(request).await?;
    // TODO - Must be QM to view this page.

    Ok(CreateQuestTemplate {
        config: &app_state.config,
    }
    .to_response())
}

#[derive(Deserialize)]
struct Slug {
    slug: String,
}

#[get("/qm/check_existing_slug")]
async fn check_existing_slug(
    app_state: web::Data<AppState>,
    slug: web::Query<Slug>,
    request: HttpRequest,
) -> impl Responder {
    let (session_info, user_id) = match app_state.get_session(request).await {
        Ok(session_info) => session_info,
        _ => return partials::FailureTemplate { text: "error" }.to_response(),
    };
    let slug = &slug.slug;

    match sqlx::query_as(
        r#"
        select exists(
          select 1
          from quest
          where slug = $1
          and questmaster = $2
          limit 1
        )
        "#,
    )
    .bind(&slug)
    .bind(&user_id)
    .fetch_one(&app_state.db_pool)
    .await
    {
        Ok((true,)) => partials::FailureTemplate {
            text: format!("You already have a quest with slug \"{slug}\"").as_str(),
        }
        .to_response(),
        Ok((false,)) => partials::SuccessTemplate {
            text: format!("\"{slug}\" is available").as_str(),
        }
        .to_response(),
        Err(_) => HttpResponse::InternalServerError().body("database error"),
    }
}

#[post("/qm/new")]
async fn create_new_quest_submit() -> Result<impl Responder> {
    Ok(HttpResponse::Ok().body(""))
}
