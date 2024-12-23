use aws_sdk_sns::{config::Region, Client as SnsClient};
use chrono::{DateTime, Utc};
use nimble::attatch_db;
use nimble::rga::rga::RGA;
use nimble::routes::*;
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;
use std::env;
use std::error::Error as StdError;
use std::sync::Arc;
use uuid::Uuid;

#[macro_use]
extern crate rocket;

#[launch]
async fn rocket() -> Result<(), Box<dyn StdError>> {
    // 1: Database connection string
    // 2. Replica ID
    let arguments: Vec<String> = env::args().collect();
    let rgas: Arc<Mutex<HashMap<Uuid, RGA>>> = Arc::new(Mutex::new(HashMap::new()));

    let config = aws_config::from_env()
        .region(Region::new("af-south-1"))
        .load()
        .await;
    let sns_client = Arc::new(Mutex::new(SnsClient::new(&config)));
    let topic_arn = std::env::var("SNS_TOPIC").expect("SNS_TOPIC must be set");
    let replica_id: i64 = match arguments.get(2) {
        Some(id) => id.parse::<i64>().unwrap(),
        None => std::process::exit(1),
    };

    let start_time: DateTime<Utc> = Utc::now();
    rocket::build()
        .attach(attatch_db())
        .manage(replica_id)
        .manage(topic_arn)
        .manage(sns_client)
        .manage(rgas)
        .manage(start_time)
        .mount(
            "/",
            routes![
                insert,
                update,
                delete,
                create_document,
                fetch_document,
                handle_sns_notification,
            ],
        )
        .launch()
        .await;

    Ok(())
}
