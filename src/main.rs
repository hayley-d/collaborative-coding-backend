use std::sync::Arc;

use nimble::rga::rga::RGA;
use nimble::routes::{delete, insert, state, update};
use nimble::{remote_delete, remote_insert, remote_update};
use rocket::tokio::sync::Mutex;

#[macro_use]
extern crate rocket;

#[launch]
fn rocket() -> _ {
    let rga = Arc::new(Mutex::new(RGA::new(1, 1)));
    rocket::build().manage(rga).mount(
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
