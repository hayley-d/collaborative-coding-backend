use chrono::{DateTime, Utc};
use nimble::rga::rga::RGA;
use nimble::routes::*;
use nimble::{attatch_db, remote_delete, remote_insert, remote_update};
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

#[macro_use]
extern crate rocket;

#[launch]
async fn rocket() -> _ {
    // 1: Database connection string
    // 2. Replica ID
    let arguements: Vec<String> = env::args().collect();
    let rgas: Arc<Mutex<HashMap<String, RGA>>> = Arc::new(Mutex::new(HashMap::new()));

    let replica_id: i64 = match arguments.get(2) {
        Some(id) => id.parse::<i64>().unwrap(),
        None => std::process::exit(1),
    };

    let start_time: DateTime<Utc> = Utc::now();
    rocket::build()
        .attatch(attatch_db())
        .manage(replica_id)
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
