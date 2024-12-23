use chrono::{DateTime, Utc};
use nimble::rga::rga::RGA;
use nimble::routes::*;
use nimble::{attatch_db, remote_delete, remote_insert, remote_update};
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[macro_use]
extern crate rocket;

#[launch]
async fn rocket() -> _ {
    let rgas: Arc<Mutex<HashMap<String, RGA>>> = Arc::new(Mutex::new(HashMap::new()));

    let start_time: DateTime<Utc> = Utc::now();
    rocket::build()
        .attatch(attatch_db())
        .manage(rgas)
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
        .launch()
        .await?;

    Ok(())
}
