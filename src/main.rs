#![allow(dead_code)]
#![allow(unused_variables)]
// Temporarily disable some warnings for development.

use crate::app_state::{AppState, CompiledRegex};
use crate::models::*;

use actix_web::{get, http, middleware, post, web, App, HttpResponse, HttpServer, Responder};
use askama_actix::Template;
use concat_arrays::concat_arrays;
use env_logger::Env;
use fred::interfaces::ClientLike;
use log::info;
use regex::Regex;
use serde::Deserialize;
use uuid::Uuid;

mod app_state;
mod models;
mod oauth;
mod routes;

#[get("/user/{username}/exists")]
async fn check_user_exists(
    app_state: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let username = path.into_inner();
    let result: Result<Option<(i32,)>, sqlx::Error> =
        sqlx::query_as("select 1 from account where username = $1 limit 1")
            .bind(username)
            .fetch_optional(&app_state.db_pool)
            .await;

    match result {
        Ok(Some(_)) => HttpResponse::Ok().finish(),
        Ok(_) => HttpResponse::NotFound().finish(),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

#[get("/user/{username}/info")]
async fn get_user(app_state: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let username = path.into_inner();
    let acct: Result<Account, sqlx::Error> = sqlx::query_as(
        r#"
        select id, username, display_name, email, bio
        from account
        where username = $1
        limit 1
        "#,
    )
    .bind(username)
    .fetch_one(&app_state.db_pool)
    .await;
    match acct {
        Ok(acct) => HttpResponse::Ok().json(acct),
        _ => HttpResponse::NotFound().finish(),
    }
}

// #[derive(Queryable, Selectable, Identifiable, Debug)]
// #[diesel(table_name = crate::schema::account)]
// #[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Debug, Deserialize)]
pub struct AccountCreateRequest {
    // pub id: uuid::Uuid,
    // pub username: String,
    pub display_name: String,
    pub email: String,
    // pub bio: Option<String>,
}

#[post("/user/{username}/create")]
async fn create_user(
    app_state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<AccountCreateRequest>,
) -> impl Responder {
    let username = path.into_inner();
    let result: Result<(Uuid,), sqlx::Error> = sqlx::query_as(
        r#"
        insert into account (id, username, display_name, email)
        values ($1, $2, $3, $4)
        returning id
        "#,
    )
    .bind(Uuid::now_v6(&app_state.uuid_seed))
    .bind(username)
    .bind(&body.display_name)
    .bind(&body.email)
    .fetch_one(&app_state.db_pool)
    .await;

    match result {
        Ok((my_uuid,)) => {
            info!("created user with id {}", my_uuid);
            HttpResponse::Ok().finish()
        }
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    name: &'a str,
}

#[get("/")]
pub async fn index() -> impl Responder {
    IndexTemplate { name: "Alice" }
}

#[get("/tailwind.css")]
pub async fn get_tailwind() -> impl Responder {
    HttpResponse::Ok()
        .content_type(http::header::ContentType(mime::TEXT_CSS))
        // Can't figure out how to use std::path::MAIN_SEPARATOR with concat!,
        // so this might not work on Windows.
        .body(include_str!(concat!(env!("OUT_DIR"), "/tailwind.css")))
}

#[actix_web::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let port: u16 = std::env::var("BACKEND_PORT").map_or(8080, |vv| vv.parse().unwrap());

    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url.as_str())
        .await?;

    let redis_config = fred::prelude::RedisConfig::from_url(redis_url.as_str())
        .expect("failed to create RedisConfig from url");
    let redis_pool = fred::prelude::RedisPool::new(redis_config, None, None, None, 5)
        .expect("failed to create RedisPool");
    redis_pool
        .init()
        .await
        .expect("failed to initialize redis connection pool");

    let uuid_seed = concat_arrays!(std::process::id().to_ne_bytes(), [0; 2]);

    let oauth_client = oauth::oauth_client(port);

    let regex = CompiledRegex {
        alphanumeric: Regex::new(r"^[0-9A-Za-z]+$").expect("failed to compile regex"),
        oauth_state_ok: Regex::new(r"^[0-9A-Za-z+/-]+=*$").expect("failed to compile regex"),
    };

    let app_state = AppState {
        db_pool,
        redis_pool,
        oauth_client,
        regex,
        uuid_seed,
    };

    HttpServer::new(move || {
        let app = App::new()
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(app_state.clone()))
            .service(get_tailwind)
            .service(check_user_exists)
            .service(get_user)
            .service(create_user)
            .service(index);
        let app = routes::add_routes(app);
        app
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await?;

    Ok(())
}
