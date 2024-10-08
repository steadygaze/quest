mod auth;
mod home;
mod markup;
mod prelude;
mod qm;
mod quest;
mod settings;

use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;
use actix_web::web;

pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    app.service(auth::add_routes(web::scope("/auth")))
        .service(markup::add_routes(web::scope("/markup")))
        .service(qm::add_routes(web::scope("/qm")))
        .service(quest::add_routes(web::scope("/@{username}")))
        .service(settings::add_routes(web::scope("/settings")))
        // We have to add the home route individually as a special exception because web::scope("")
        // interferes with other "/" routes for some reason.
        .service(home::index)
}
