mod edit;
mod list;
mod new;

use actix_web::dev::ServiceFactory;
use actix_web::dev::ServiceRequest;

pub fn add_routes(scope: actix_web::Scope) -> actix_web::Scope {
    let scope = edit::add_routes(scope);
    let scope = list::add_routes(scope);
    let scope = new::add_routes(scope);
    scope
}
