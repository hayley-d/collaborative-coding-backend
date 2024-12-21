use std::sync::Arc;

use nimble::rga::rga::RGA;
use nimble::routes::{delete, insert, state, update};
use rocket::tokio::sync::Mutex;

#[macro_use]
extern crate rocket;

#[launch]
fn rocket() -> _ {
    let rga = Arc::new(Mutex::new(RGA::new(1, 1)));
    rocket::build()
        .manage(rga)
        .mount("/", routes![insert, update, delete, state])
}
