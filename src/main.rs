use aws_sdk_sns::{config::Region, Client as SnsClient, Config};
use chrono::{DateTime, Utc};
use nimble::rga::rga::RGA;
use nimble::routes::*;
use nimble::{attatch_db, remote_delete, remote_insert, remote_update};
use rocket::fairing::AdHoc;
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use uuid::Uuid;

#[macro_use]
extern crate rocket;

#[launch]
async fn rocket() -> _ {
    // 1: Database connection string
    // 2. Replica ID
    let arguements: Vec<String> = env::args().collect();
    let rgas: Arc<Mutex<HashMap<Uuid, RGA>>> = Arc::new(Mutex::new(HashMap::new()));

    let config = aws_config::from_env()
        .region(Region::new("af-south-1"))
        .load()
        .await;
    let sns_client = Arc::new(Mutex::new(SnsClient::new(&config)));

    let replica_id: i64 = match arguments.get(2) {
        Some(id) => id.parse::<i64>().unwrap(),
        None => std::process::exit(1),
    };

    let start_time: DateTime<Utc> = Utc::now();
    rocket::build()
        .attatch(attatch_db())
        .manage(replica_id)
        .manage(sns_client)
        .manage(rgas)
        .manage(start_time)
        .mount(
            "/",
            routes![
                /*insert,
                update,
                delete,
                remote_insert,
                remote_update,
                remote_delete,
                state*/
                create_document
            ],
        )
        .launch()
        .await?;

    Ok(())
}
