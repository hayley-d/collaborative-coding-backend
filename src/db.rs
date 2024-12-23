use crate::ApiError;
use rocket::fairing::AdHoc;
use rocket::tokio;
use rocket::tokio::sync::Mutex;
use std::sync::Arc;
use tokio_postgres::{Client, NoTls};

/// Fairing for managing the PostgreSQL client in rocket's state
pub fn attatch_db() -> AdHoc {
    AdHoc::on_ignite("Attatch DB", |rocket| async {
        match connect_to_db().await {
            Ok(client) => rocket.manage(Arc::new(Mutex::new(client))),
            Err(e) => {
                eprintln!("Failed to initialize DB: {:?}", e);
                std::process::exit(1);
            }
        }
    })
}

/// Connects to the AWS RDS instance
pub async fn connect_to_db() -> Result<Client, ApiError> {
    let host = std::env::var("DB_HOST").expect("DB_HOST must be set");
    let user = std::env::var("DB_USER").expect("DB_USER must be set");
    let password = std::env::var("DB_PSW").expect("DB_PSW must be set");
    let dbname = std::env::var("DB_NAME").expect("DB_NAME must be set");

    let config = format!(
        "host={} user={} password={} dbname={}",
        host, user, password, dbname
    );

    let (client, connection) = tokio_postgres::connect(&config, NoTls)
        .await
        .map_err(|e| ApiError::DatabaseError(e.to_string()))?;

    tokio::spawn(async move { connection.await });
    return Ok(client);
}

/// Sends a SNS message
pub async fn send_sns_notification(
    message: &str,
    sns_topic: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = aws_config::load_from_env().await;
    let sns_client = aws_sdk_sns::Client::new(&config);

    match sns_client
        .publish()
        .message(message)
        .topic_arn(sns_topic)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}
