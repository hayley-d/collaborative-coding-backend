use chrono::{DateTime, Utc};
use nimble::rga::rga::RGA;
use nimble::{remote_delete, remote_insert, remote_update};
use nimble::{routes::*, Database};
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[macro_use]
extern crate rocket;

#[launch]
fn rocket() -> _ {
    let rgas: Arc<Mutex<HashMap<String, RGA>>> = Arc::new(Mutex::new(HashMap::new()));
    let db: Arc<Mutex<Database>> = Arc::new(Mutex::new(Database {}));
    let start_time: DateTime<Utc> = Utc::now();
    rocket::build()
        .manage(rgas)
        .manage(db)
        .manage(start_time)
        .mount(
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
