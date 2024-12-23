use crate::rga::rga::Operation;
use chrono::Utc;
use rocket::fairing::AdHoc;
use rocket::tokio;
use rocket::tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, NoTls};

/// Represents an operation stored in the Operations table.
#[derive(Debug, Serialize, Deserialize)]
pub struct DbOperation {
    pub operation_id: i32,
    pub replica_id: i32,
    pub document_id: String,
    pub datetime: String,  // ISO 8601 format
    pub operation: String, // Insert, Update, or Delete
    pub s4vector: Value,   // JSON representation of S4Vector
    pub value: Option<String>,
    pub tombstone: bool,
}

/// Represents a document snapshot stored as tuples of (S4Vector, Value, Tombstone).
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub document_id: String,
    pub s4vector: Value, // JSON representation of S4Vector
    pub value: String,
    pub tombstone: bool,
}

impl DbOperation {
    /// Inserts an operation into the Operations table.
    pub async fn insert_into_db(&self, client: &Client) -> Result<(), tokio_postgres::Error> {
        let query = r#"INSERT INTO operations (replica_id,document_id,datetime,operation,s4vector,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7)"#;
        client
            .execute(
                query,
                &[
                    &self.replica_id,
                    &self.document_id,
                    &self.datetime,
                    &self.operation,
                    &self.s4vector,
                    &self.value,
                    &self.tombstone,
                ],
            )
            .await?;
        Ok(())
    }
}

impl DocumentSnapshot {
    /// Inserts or updates a document snapshot in the Document Snapshot table.
    pub async fn upsert_into_db(&self, client: &Client) -> Result<(), tokio_postgres::Error> {
        let query = r#"INSERT INTO document_snapshots (document_id,s4vector,value,tombstone) VALUES ($1,$2,$3,$4) ON CONFLICT (document_id,s4vector) DO UPDATE SET value = $3, tombstone = $4"#;

        client
            .execute(
                query,
                &[
                    &self.document_id,
                    &self.s4vector,
                    &self.value,
                    &self.tombstone,
                ],
            )
            .await?;
        Ok(())
    }
}

/// Initialize the database connection securely.
pub async fn initialize_db() -> Result<Client, tokio_postgres::Error> {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");

    let (_, connection) = tokio_postgres::connect(&database_url, NoTls).await?;

    // Spawn the connection handling task
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {:?}", e);
        }
    });
}

/// Fairing for managing the PostgreSQL client in rocket's state
pub fn attatch_db() -> AdHoc {
    AdHoc::on_ignite("Attatch DB", |rocket| async {
        match initialize_db().await {
            Ok(client) => rocket.manage(Arc::new(Mutex::new(client))),
            Err(e) => {
                eprintln!("Failed to initialize DB: {:?}", e);
                std::process::exit(1);
            }
        }
    })
}

/// Process a local operation
pub async fn process_local_operation(
    operation: DbOperation,
    snapshot: DocumentSnapshot,
    client: &Client,
) -> Result<(), tokio_postgres::Error> {
    // Insert operation into the Operations table
    operation.insert_into_db(client).await?;
    snapshot.upsert_into_db(client).await?;
    Ok(())
}
