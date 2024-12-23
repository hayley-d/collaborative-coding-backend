use crate::rga::rga::RGA;
use crate::{
    db, ApiError, BroadcastOperation, CreateDocumentRequest, CreateDocumentResponse,
    DocumentSnapshot, OperationRequest, S4Vector, SnsNotification,
};
use aws_sdk_sns::Client as SnsClient;
use rocket::serde::json::Json;
use rocket::tokio::sync::Mutex;
use rocket::{get, post};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_postgres::Client;
use uuid::Uuid;

/// This module implements routes for managing collaborative documents
/// using RGAs (Replicated Growable Arrays) in a distributed system.
/// It includes functionality to fetch documents, synchronize with the
/// database, manage CRDT operations, and monitor replica health.

/// Shared state type: Maps document IDs to their corresponding RGA instances.
type SharedRGAs = Arc<Mutex<HashMap<Uuid, RGA>>>;

/// Route to create a new document
///
/// This route inserts metadata for a new document into the database, including
/// the document's creation date, owner ID, and title. It also creates an initial
/// snapshot and logs the operation into the database, all wrapped in a transaction
/// to ensure atomicity and consistency. The response will return the document ID
/// of the newly created document and a success message.
/// Example Request
/// {
///     "owner_id": "550e8400-e29b-41d4-a716-446655440000",
///     "title": "My New Document"
/// }
///
/// Example Respose
/// {
///     "document_id" : "f47ac10b-58cc-4372-a567-0e02b2c3d479",
///     "message" : "Document f47ac10b-58cc-4372-a567-0e02b2c3d479 created successfully"
/// }

#[post("/create_document", format = "json", data = "<request>")]
pub async fn create_document(
    request: Json<CreateDocumentRequest>,
    replica_id: &rocket::State<Arc<Mutex<i64>>>,
    db: &rocket::State<Arc<Mutex<Client>>>,
) -> Result<Json<CreateDocumentResponse>, ApiError> {
    // Lock the database client and replica ID for usage
    let mut client = db.lock().await;
    let replica_id: i64 = *replica_id.lock().await;

    // Default to "New docuement" if the title is empty
    let title = if request.title.to_string().is_empty() {
        String::from("New document")
    } else {
        request.title.to_string()
    };

    // Get the current timestamp (in ISO 8601 format) for the document creation
    let create_date = chrono::Utc::now().to_rfc3339();

    // Initial content for the document
    let initial_content = String::new();

    // SQL query to insert the document metadata into the documents table
    let document_query = r#"INSERT INTO document (owner_id,creation_date,title) VALUES ($1,$2,$3) RETURNING document_id"#;
    // Execute the query and retrueve the document_id (UUID) for the new document
    let document_id: Uuid = client
        .query_one(document_query, &[&request.owner_id, &create_date, &title])
        .await
        .map_err(|e| {
            ApiError::DatabaseError(format!(
                "Failed to insert document metadata into the documents table: {}",
                e.to_string()
            ))
        })?
        .get(0);

    // Start Database trasaction to ensure atomicity
    let tx = client.transaction().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to start transaction: {}", e.to_string()))
    })?;

    // SQL query to insert a new snapshot into the document_snapshots table
    let snapshot_query = r#"INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7)"#;
    // SQL query to insert a new operation into the operations table
    let operation_query = r#"INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#;

    // Execute the snapshot insert query
    tx.execute(
        snapshot_query,
        &[
            &document_id,
            &(0 as i32),
            &(0 as i32),
            &replica_id,
            &(0 as i32),
            &initial_content,
            &false,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert document snapshot into the document_snapshots table: {}",
            e.to_string()
        ))
    })?;

    let timestamp = chrono::Utc::now().to_rfc3339().to_string();

    // Execute the operation insert query
    tx.execute(
        operation_query,
        &[
            &document_id,
            &(0 as i32),
            &(0 as i32),
            &replica_id,
            &(0 as i32),
            &Some(initial_content.clone()),
            &false,
            &timestamp,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert operation into the operations table: {}",
            e.to_string()
        ))
    })?;

    // Commit the transaction to persist the changes
    tx.commit().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to commit transaction: {}", e.to_string()))
    })?;

    // Return a response indicating the document was created successfully.
    return Ok(Json(CreateDocumentResponse {
        document_id,
        message: format!("Document {} created successuflly", document_id),
    }));
}

/// Fetch a document from the Aurora DB and initialize an RGA.
#[get("/document/<id>")]
pub async fn fetch_document(
    id: String,
    rgas: &rocket::State<SharedRGAs>,
    replica_id: &rocket::State<Arc<Mutex<i64>>>,
    db: &rocket::State<Arc<Mutex<Client>>>,
) -> Result<(), ApiError> {
    let document_id: Uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::RequestFailed(format!("Failed to parse document id")))?;

    let mut rgas = rgas.lock().await;
    let client = db.lock().await;

    // Check if the document has already been loaded into the hashmap
    if rgas.contains_key(&document_id) {
        return Ok(());
    }

    let query =
        r#"SELECT * from document_snapshots WHERE document_id=$1 ORDER BY ssn, sum, sid,seq;"#;

    let rows = client.query(query, &[&document_id]).await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to find document in database: {:?}", e))
    })?;

    let snapshots: Vec<DocumentSnapshot> = rows
        .iter()
        .map(|row| DocumentSnapshot {
            document_id: row.get(0),
            ssn: row.get(1),
            sum: row.get(2),
            sid: row.get(3),
            seq: row.get(4),
            value: row.get(5),
            tombstone: row.get(6),
        })
        .collect();

    let mut rga = RGA::new(*(replica_id.lock().await) as u64, 1);

    for operation in snapshots {
        let s4 = S4Vector {
            ssn: operation.ssn as u64,
            sum: operation.sum as u64,
            sid: operation.sid as u64,
            seq: operation.seq as u64,
        };

        rga.remote_insert(operation.value, s4, None, None).await;
    }

    rgas.insert(document_id, rga);

    return Ok(());
}

/// Insert a value into the RGA of a specific document.
/* pub struct OperationRequest {
    value: Option<String>,
    s4vector: Option<S4Vector>,
    tombstone: bool,
    left: Option<S4Vector>,
    right: Option<S4Vector>,
}*/

#[post("/document/<id>/insert", format = "json", data = "<request>")]
pub async fn insert(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
    db: &rocket::State<Arc<Mutex<Client>>>,
    sns_client: &rocket::State<Arc<Mutex<SnsClient>>>,
    topic: &rocket::State<Arc<Mutex<String>>>,
) -> Result<(), ApiError> {
    let document_id: Uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::RequestFailed(format!("Failed to parse document id")))?;

    let mut rgas = rgas.lock().await;
    let mut client = db.lock().await;

    // Check if the document has been loaded
    let rga: &mut RGA = match rgas.get_mut(&document_id) {
        Some(r) => r,
        None => return Err(ApiError::RequestFailed(String::from("Document not found"))),
    };

    let value: String = if request.value.is_some() {
        request.value.clone().unwrap()
    } else {
        return Err(ApiError::RequestFailed(format!("Value not found")));
    };

    let mut op: BroadcastOperation = match rga
        .local_insert(value.clone(), request.left, request.right, document_id)
        .await
    {
        Ok(obj) => obj,
        Err(_) => {
            return Err(ApiError::RequestFailed(format!(
                "Error inserting into file"
            )))
        }
    };

    op.document_id = document_id;

    let s4 = op.s4vector();

    let operation_query = r#"INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#;
    let snapshot_query = r#"INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7)"#;

    let current_time = chrono::Utc::now().to_rfc3339().to_string();

    let tx = client.transaction().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to create transaction: {:?}", e.to_string()))
    })?;

    tx.execute(
        operation_query,
        &[
            &document_id,
            &(s4.ssn as i64),
            &(s4.sum as i64),
            &(s4.sid as i64),
            &(s4.seq as i64),
            &value,
            &false,
            &current_time,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert into operations table: {:?}",
            e.to_string()
        ))
    })?;

    tx.execute(
        snapshot_query,
        &[
            &document_id,
            &(s4.ssn as i64),
            &(s4.sum as i64),
            &(s4.sid as i64),
            &(s4.seq as i64),
            &value,
            &false,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert into document_snapshot table: {:?}",
            e.to_string()
        ))
    })?;

    tx.commit().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to commit transaction: {:?}", e.to_string()))
    })?;

    //Broadcast to SNS
    match db::send_operation(Arc::clone(sns_client), &topic.lock().await, &op).await {
        Ok(_) => (),
        Err(_) => {
            return Err(ApiError::DatabaseError(format!(
                "Failed to send SNS notification"
            )))
        }
    };

    return Ok(());
}

#[post("/document/<id>/update", format = "json", data = "<request>")]
pub async fn update(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
    db: &rocket::State<Arc<Mutex<Client>>>,
    sns_client: &rocket::State<Arc<Mutex<SnsClient>>>,
    topic: &rocket::State<Arc<Mutex<String>>>,
) -> Result<(), ApiError> {
    let document_id: Uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::RequestFailed(format!("Failed to parse document id")))?;

    let mut rgas = rgas.lock().await;
    let mut client = db.lock().await;

    // Check if the document has been loaded
    let rga: &mut RGA = match rgas.get_mut(&document_id) {
        Some(r) => r,
        None => return Err(ApiError::RequestFailed(String::from("Document not found"))),
    };

    let value: String = if request.value.is_some() {
        request.value.clone().unwrap()
    } else {
        return Err(ApiError::RequestFailed(format!("Value not found")));
    };

    let mut op: BroadcastOperation = match rga
        .local_update(request.s4vector.unwrap(), value.clone(), document_id)
        .await
    {
        Ok(obj) => obj,
        Err(_) => return Err(ApiError::RequestFailed(format!("Error updating file"))),
    };

    op.document_id = document_id;

    let s4 = op.s4vector();

    let operation_query = r#"INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#;
    let snapshot_query = r#"INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (document_id,ssn,sum,sid,seq) DO UPDATE set value = EXCLUDED.value, tombstone = EXCLUDED.tombstone"#;

    let current_time = chrono::Utc::now().to_rfc3339().to_string();

    let tx = client.transaction().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to create transaction: {:?}", e.to_string()))
    })?;

    tx.execute(
        operation_query,
        &[
            &document_id,
            &(s4.ssn as i64),
            &(s4.sum as i64),
            &(s4.sid as i64),
            &(s4.seq as i64),
            &value,
            &false,
            &current_time,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert into operations table: {:?}",
            e.to_string()
        ))
    })?;

    tx.execute(
        snapshot_query,
        &[
            &document_id,
            &(s4.ssn as i64),
            &(s4.sum as i64),
            &(s4.sid as i64),
            &(s4.seq as i64),
            &value,
            &false,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to update into document_snapshot table: {:?}",
            e.to_string()
        ))
    })?;

    tx.commit().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to commit transaction: {:?}", e.to_string()))
    })?;

    //Broadcast to SNS
    match db::send_operation(Arc::clone(sns_client), &topic.lock().await, &op).await {
        Ok(_) => (),
        Err(_) => {
            return Err(ApiError::DatabaseError(format!(
                "Failed to send SNS notification"
            )))
        }
    };

    return Ok(());
}

#[post("/document/<id>/delete", format = "json", data = "<request>")]
pub async fn delete(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
    db: &rocket::State<Arc<Mutex<Client>>>,
    sns_client: &rocket::State<Arc<Mutex<SnsClient>>>,
    topic: &rocket::State<Arc<Mutex<String>>>,
) -> Result<(), ApiError> {
    let document_id: Uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::RequestFailed(format!("Failed to parse document id")))?;

    let mut rgas = rgas.lock().await;
    let mut client = db.lock().await;

    // Check if the document has been loaded
    let rga: &mut RGA = match rgas.get_mut(&document_id) {
        Some(r) => r,
        None => return Err(ApiError::RequestFailed(String::from("Document not found"))),
    };

    let mut op: BroadcastOperation = match rga
        .local_delete(request.s4vector.unwrap(), document_id)
        .await
    {
        Ok(obj) => obj,
        Err(_) => return Err(ApiError::RequestFailed(format!("Error updating file"))),
    };

    op.document_id = document_id;

    let s4 = op.s4vector();

    let operation_query = r#"INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#;
    let snapshot_query = r#"INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (document_id,ssn,sum,sid,seq) DO UPDATE set value = EXCLUDED.value, tombstone = EXCLUDED.tombstone"#;

    let current_time = chrono::Utc::now().to_rfc3339().to_string();

    let tx = client.transaction().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to create transaction: {:?}", e.to_string()))
    })?;

    tx.execute(
        operation_query,
        &[
            &document_id,
            &(s4.ssn as i64),
            &(s4.sum as i64),
            &(s4.sid as i64),
            &(s4.seq as i64),
            &"",
            &false,
            &current_time,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert into operations table: {:?}",
            e.to_string()
        ))
    })?;

    tx.execute(
        snapshot_query,
        &[
            &document_id,
            &(s4.ssn as i64),
            &(s4.sum as i64),
            &(s4.sid as i64),
            &(s4.seq as i64),
            &"",
            &false,
        ],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to update document_snapshot table: {:?}",
            e.to_string()
        ))
    })?;

    tx.commit().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to commit transaction: {:?}", e.to_string()))
    })?;

    //Broadcast to SNS
    match db::send_operation(Arc::clone(sns_client), &topic.lock().await, &op).await {
        Ok(_) => (),
        Err(_) => {
            return Err(ApiError::DatabaseError(format!(
                "Failed to send SNS notification"
            )))
        }
    };

    return Ok(());
}

// Receives SNS notifications to perform remote operations
#[post("/sns", format = "json", data = "<notification>")]
pub async fn handle_sns_notification(
    notification: Json<SnsNotification>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rags = rgas.lock().await;

    let operation: BroadcastOperation = serde_json::from_str(&notification.0.message)
        .map_err(|_| ApiError::InternalServerError(format!("Failed to parse SNS message")))?;

    let rga = rags.get_mut(&operation.document_id);

    let rga = match rga {
        Some(r) => r,
        None => {
            return Err(ApiError::RequestFailed(format!("Document not loaded")));
        }
    };

    match operation.operation.as_str() {
        "Insert" => {
            let _ = &rga
                .remote_insert(
                    operation.value.clone().unwrap(),
                    operation.s4vector(),
                    operation.left,
                    operation.right,
                )
                .await;
        }
        "Update" => {
            rga.remote_update(operation.s4vector(), operation.value.unwrap())
                .await;
        }
        "Delete" => {
            rga.remote_delete(operation.s4vector()).await;
        }
        _ => return Err(ApiError::RequestFailed(format!("Invalid operation"))),
    }

    return Ok(());
}
