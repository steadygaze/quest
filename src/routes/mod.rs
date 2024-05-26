pub mod oauth;
pub mod qm;
pub mod quest;

use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    let app = oauth::add_routes(app);
    let app = qm::add_routes(app);
    let app = quest::add_routes(app);
    app
}
