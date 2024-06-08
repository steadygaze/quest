mod view;

use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    let scope = view::add_routes(scope);
    scope
}
