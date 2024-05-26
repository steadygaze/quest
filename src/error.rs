use std::backtrace::Backtrace;
use std::fmt::Display;

/// Error handling code, specifically for actix-web. Without this, we won't be
/// able to use '?' to return errors and will have to construct responses for
/// all of them manually. Some general info on error handling in actix-web can
/// be found at:
/// https://woile.github.io/actix-website/docs/errors/
use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    middleware::ErrorHandlerResponse,
    HttpResponse,
};
use askama::Template;
use fred::error::RedisError;

/// Common errors that can be unwrapped in handlers.
#[derive(Debug)]
enum QuestServerError {
    DatabaseError(sqlx::Error),
    RedisError(RedisError),
}

impl Display for QuestServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if cfg!(debug_assertions) {
            match self {
                QuestServerError::DatabaseError(err) => {
                    write!(f, "Database error: {}", err)?;
                }
                QuestServerError::RedisError(err) => {
                    write!(f, "Redis error: {}", err)?;
                }
            }
        } else {
            write!(f, "An internal error occurred.")?;
        }
        Ok(())
    }
}

mod filters {
    pub fn nonetoempty(s: Option<&str>) -> askama::Result<&str> {
        match s {
            Some(str) => Ok(str),
            None => Ok(""),
        }
    }
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate<'a> {
    pub backtrace: &'a Backtrace,
    pub status: &'a StatusCode,
    pub message: &'a String,
}

impl error::ResponseError for QuestServerError {
    fn error_response(&self) -> HttpResponse {
        let backtrace = if cfg!(debug_assertions) {
            Backtrace::force_capture()
        } else {
            Backtrace::disabled()
        };
        let status_code = self.status_code();
        HttpResponse::build(status_code)
            .content_type(ContentType::html())
            .body(
                ErrorTemplate {
                    backtrace: &backtrace,
                    status: &status_code,
                    message: &self.to_string(),
                }
                .to_string(),
            )
    }

    fn status_code(&self) -> StatusCode {
        match self {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Magic function to serve a custom 404 page.
pub fn custom_404<B>(
    res: actix_web::dev::ServiceResponse<B>,
) -> actix_web::Result<ErrorHandlerResponse<B>> {
    // Decompose the existing response, override the body, and reconstruct it.
    let (req, res) = res.into_parts();
    let res = res.set_body(
        ErrorTemplate {
            backtrace: &Backtrace::disabled(),
            status: &StatusCode::NOT_FOUND,
            message: &format!("The page \"{}\" doesn't exist.", req.path()),
        }
        .to_string(),
    );

    let res = actix_web::dev::ServiceResponse::new(req, res)
        .map_into_boxed_body()
        .map_into_right_body();

    Ok(ErrorHandlerResponse::Response(res))
}
