#![allow(dead_code)]
#![allow(unused_variables)]
// Temporarily disable some warnings for development.

use crate::app_state::{AppConfig, AppState, CompiledRegex};

use actix_web::HttpRequest;
use actix_web::{get, http, middleware, web, App, HttpResponse, HttpServer, Responder};
use actix_web_static_files::ResourceFiles;
use askama_actix::Template;
use askama_actix::TemplateToResponse;
use concat_arrays::concat_arrays;
use env_logger::Env;
use fred::interfaces::ClientLike;
use listenfd::ListenFd;
use log::warn;
use regex::Regex;
use std::sync::Arc;
use std::thread;

mod app_state;
mod error;
mod key;
mod oauth;
mod partials;
mod routes;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    config: &'a AppConfig,
    name: &'a str,
}

#[get("/")]
pub async fn index(app_state: web::Data<AppState>, request: HttpRequest) -> impl Responder {
    log::trace!("Got sid cookie {:?}", request.cookie("sid"));
    IndexTemplate {
        config: &app_state.config,
        name: "Alice",
    }
    .to_response()
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
    env_logger::init_from_env(Env::default().default_filter_or("info,quest=trace"));

    let config: AppConfig = app_state::config_with_defaults()
        .unwrap()
        .add_source(
            config::Environment::with_prefix("QUEST"), // .try_parsing(true)
                                                       // .list_separator(","),
        )
        .build()
        .expect("failed to build app config")
        .try_deserialize()
        .expect("failed to parse app config");

    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(config.database_url.as_str())
        .await?;

    let redis_config = fred::prelude::RedisConfig::from_url(config.redis_url.as_str())
        .expect("failed to create RedisConfig from url");
    let redis_pool = fred::prelude::RedisPool::new(redis_config, None, None, None, 5)
        .expect("failed to create RedisPool");
    redis_pool
        .init()
        .await
        .expect("failed to initialize redis connection pool");

    let uuid_seed = concat_arrays!(std::process::id().to_ne_bytes(), [0; 2]);

    let oauth_client = oauth::oauth_client(&config);

    let regex = CompiledRegex {
        alphanumeric: Regex::new(r"^[0-9A-Za-z]+$").expect("failed to compile regex"),
        oauth_state_ok: Regex::new(r"^[0-9A-Za-z+/_-]+=*$").expect("failed to compile regex"),
    };

    let (sender, receiver) = crossbeam_channel::unbounded::<app_state::FnBox>();
    thread::spawn(move || async move {
        loop {
            match receiver.recv() {
                Ok(fn_box) => fn_box(),
                Err(err) => {
                    warn!("Error dequeueing background work: {err}");
                }
            }
        }
    });

    let port = config.port.clone();
    let app_state = AppState {
        background_sender: Arc::new(sender),
        config,
        db_pool,
        redis_pool,
        oauth_client,
        regex,
        uuid_seed,
    };

    let mut listenfd = ListenFd::from_env();
    let server = HttpServer::new(move || {
        let generated = generate();
        let app = App::new()
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .wrap(
                middleware::ErrorHandlers::new()
                    .handler(http::StatusCode::NOT_FOUND, error::custom_404),
            )
            .app_data(web::Data::new(app_state.clone()))
            .service(
                ResourceFiles::new("/", generated)
                    // Not useful because we have no static HTML files.
                    .do_not_resolve_defaults()
                    // Required when mounted on "/", otherwise all other
                    // handlers are skipped.
                    .skip_handler_when_not_found(),
            )
            .service(get_tailwind)
            .service(index);
        let app = routes::add_routes(app);
        app
    });

    let server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
        server.listen(l)?
    } else {
        server.bind(("127.0.0.1", port))?
    };

    server.run().await?;

    Ok(())
}
