use nimble::rga::rga::RGA;
use nimble::routes::*;
use nimble::{remote_delete, remote_insert, remote_update};
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[macro_use]
extern crate rocket;

#[launch]
fn rocket() -> _ {
    let rgas: Arc<Mutex<HashMap<String, RGA>>> = Arc::new(Mutex::new(HashMap::new()));
    rocket::build().manage(rgas).mount(
        "/",
        routes![
            insert,
            update,
            delete,
            remote_insert,
            remote_update,
            remote_delete,
            state
        ],
    )
}
