mod new;

use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    new::add_routes(scope)
}
