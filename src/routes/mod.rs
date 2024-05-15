pub mod oauth;

// use actix_web::dev::HttpServiceFactory;
use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

pub fn add_routes<T>(app: actix_web::App<T>) -> actix_web::App<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = actix_web::Error, InitError = ()>,
{
    oauth::add_routes(app)
}
