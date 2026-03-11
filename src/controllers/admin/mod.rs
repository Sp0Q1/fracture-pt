pub mod engagement;

use loco_rs::prelude::*;

pub fn routes() -> Routes {
    let mut routes = Routes::new().prefix("/admin");
    for r in engagement::route_list() {
        routes = routes.add(&r.0, r.1);
    }
    routes
}
