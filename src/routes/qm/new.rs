use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

use crate::partials;
use crate::routes::prelude::*;

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
    current_profile: &'a Option<ProfileRenderInfo>,
}

#[get("/new")]
async fn create_new_quest_form(
    app_state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<impl Responder> {
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;
    // TODO - Must be QM to view this page.

    Ok(NewQuestTemplate {
        config: &app_state.config,
        current_profile: &current_profile,
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
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = match app_state.require_session(request).await {
        Ok(info) => info,
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
    let SessionInfo {
        account_id,
        current_profile,
        ..
    } = app_state.require_session(request).await?;
    // TODO - Must be QM to view this page.

    let mut transaction = app_state
        .db_pool
        .begin()
        .await
        .context("Failed to create transaction")?;

    if let (true,) = sqlx::query_as(
        r#"
        select exists (
            select 1 from quest
            where questmaster = $1 and slug = $2
            limit 1
        )
        "#,
    )
    .bind(account_id)
    .bind(&form.slug)
    .fetch_one(&mut *transaction)
    .await
    .context("Failed to check if quest exists")?
    {
        return Err(Error::AppError(format!(
            "Quest with slug {} already exists",
            form.slug
        )));
    }

    sqlx::query(
        r#"
        insert into quest (id, questmaster, title, slug)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::now_v6(&app_state.uuid_seed))
    .bind(account_id)
    .bind(&form.title)
    .bind(&form.slug)
    .execute(&mut *transaction)
    .await
    .context("Failed to insert new quest")?;

    transaction.commit().await.context("Failed to commit")?;
    Ok(HttpResponse::Ok().body("created new quest"))
    // TODO - Redirect to new quest editing view.
}
