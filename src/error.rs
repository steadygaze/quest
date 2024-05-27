use std::backtrace::{Backtrace, BacktraceStatus};
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

/// Common errors that can be unwrapped in handlers.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    // We can't use #[error] because of special handling for InternalError.
    InternalError(#[from] anyhow::Error),
    AuthError(String),
    AppError(String),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if cfg!(debug_assertions) {
            match self {
                // For security reasons, we shouldn't show detailed error
                // messages in production:
                // https://owasp.org/www-community/Improper_Error_Handling
                Error::InternalError(err) => {
                    if cfg!(debug_assertions) {
                        write!(f, "Internal error: {:?}", err)?;
                    } else {
                        write!(f, "Internal error: {}", err)?;
                    }
                }
                Error::AuthError(err) => {
                    write!(f, "{err}")?;
                }
                Error::AppError(err) => {
                    write!(f, "{err}")?;
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

impl error::ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        // let backtrace = &Backtrace::force_capture();
        let backtrace = if let Error::InternalError(anyhow_err) = &self {
            &anyhow_err.backtrace()
        } else {
            &Backtrace::disabled()
        };
        let status_code = self.status_code();
        HttpResponse::build(status_code)
            .content_type(ContentType::html())
            .body(
                ErrorTemplate {
                    backtrace,
                    status: &status_code,
                    message: &self.to_string(),
                }
                .to_string(),
            )
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Error::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::AuthError(_) => StatusCode::UNAUTHORIZED,
            Error::AppError(_) => StatusCode::BAD_REQUEST,
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
