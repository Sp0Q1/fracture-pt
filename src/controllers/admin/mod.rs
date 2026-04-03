pub mod engagement;
pub mod finding;
pub mod report;

use loco_rs::prelude::*;

pub fn routes() -> Routes {
    let mut routes = Routes::new().prefix("/admin");
    for r in engagement::route_list() {
        routes = routes.add(&r.0, r.1);
    }
    for r in finding::route_list() {
        routes = routes.add(&r.0, r.1);
    }
    for r in report::route_list() {
        routes = routes.add(&r.0, r.1);
    }
    routes
}
