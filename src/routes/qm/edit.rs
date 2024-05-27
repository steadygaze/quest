use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;
use actix_web::get;
use actix_web::post;
use actix_web::HttpResponse;
use actix_web::Responder;

use crate::error::Result;

pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    app.service(create_new_quest_form)
        .service(create_new_quest_submit)
}

#[get("/qm/new")]
async fn create_new_quest_form() -> Result<impl Responder> {
    Ok(HttpResponse::Ok().body(""))
}

#[post("/qm/new")]
async fn create_new_quest_submit() -> Result<impl Responder> {
    Ok(HttpResponse::Ok().body(""))
}
