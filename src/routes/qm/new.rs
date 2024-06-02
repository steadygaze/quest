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
use uuid::Uuid;

use crate::app_state::AppConfig;
use crate::app_state::AppState;
use crate::error::Error;
use crate::error::Result;
use crate::partials;
use crate::session::get_session_info;
use crate::session::SESSION_ID_COOKIE;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(create_new_quest_form)
        .service(create_new_quest_submit)
        .service(check_existing_slug)
}

#[derive(Template)]
#[template(path = "qm/new.html")]
struct NewQuestTemplate<'a> {
    config: &'a AppConfig,
}

#[get("/new")]
async fn create_new_quest_form(
    app_state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let (session_info, account_id) = app_state.get_session(request).await?;
    // TODO - Must be QM to view this page.

    Ok(NewQuestTemplate {
        config: &app_state.config,
    }
    .to_response())
}

#[derive(Deserialize)]
struct Slug {
    slug: String,
}

#[get("/check_existing_slug")]
async fn check_existing_slug(
    app_state: web::Data<AppState>,
    slug: web::Query<Slug>,
    request: HttpRequest,
) -> impl Responder {
    let (session_info, account_id) = match app_state.get_session(request).await {
        Ok(tup) => tup,
        // This is for injection via HTMX, so we can't show a full error page.
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
    .bind(&account_id)
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

#[derive(Deserialize)]
struct NewQuestForm {
    title: String,
    slug: String,
}

#[post("/new")]
async fn create_new_quest_submit(
    app_state: web::Data<AppState>,
    form: web::Form<NewQuestForm>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let (session_info, account_id) = app_state.get_session(request).await?;
    // TODO - Must be QM to view this page.

    sqlx::query_as(
        r#"
        insert into quest (id, questmaster, title, slug)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::now_v6(&app_state.uuid_seed))
    .bind(account_id)
    .bind(&form.title)
    .bind(&form.slug)
    .fetch_one(&app_state.db_pool)
    .await
    .context("Failed to insert new quest")?;
    Ok(HttpResponse::Ok().body("ok"))
}
