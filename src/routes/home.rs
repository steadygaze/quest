use crate::routes::prelude::*;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    config: &'a AppConfig,
    current_profile: &'a Option<ProfileRenderInfo>,
    logged_in: bool,
}

#[get("/")]
pub async fn index(app_state: web::Data<AppState>, request: HttpRequest) -> Result<impl Responder> {
    let session_info_option = app_state.get_session(request).await.transpose()?;
    let logged_in = session_info_option.is_some();
    let current_profile = &session_info_option.and_then(|x| x.current_profile);

    Ok(IndexTemplate {
        config: &app_state.config,
        current_profile,
        logged_in,
    }
    .to_response())
}
