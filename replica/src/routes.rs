use crate::rga::rga::RGA;
use crate::{
    db, ApiError, BroadcastOperation, CreateDocumentRequest, CreateDocumentResponse,
    DocumentSnapshot, OperationRequest, S4Vector, SnsNotification,
};
use aws_sdk_sns::Client as SnsClient;
use log::{error, info};
use rocket::serde::json::Json;
use rocket::tokio::sync::Mutex;
use rocket::{get, post};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_postgres::Client;
use uuid::Uuid;

/// This module defines the API routes for a collaborative coding backend system.
/// It handles document operations (insert, update, delete), document management (fetch, load),
/// and broadcasting changes via SNS (Amazon Simple Notification Service).
///
/// # Features
/// **Insert, Update, Delete**: CRUD operations for managing text collaboratively.
/// **Fetch, Load**: Retrieve and initialize document snapshots.
/// **SNS Integration**: Broadcasts changes to other replicas.

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
    let mut client = db.lock().await;
    let replica_id: i64 = *replica_id.lock().await;

    let title = if request.title.to_string().is_empty() {
        String::from("New document")
    } else {
        request.title.to_string()
    };

    let create_date = chrono::Utc::now().to_rfc3339();
    let initial_content = String::new();
    let document_query = match client.prepare("INSERT INTO document (owner_id,creation_date,title) VALUES ($1,$2,$3) RETURNING document_id").await{
        Ok(dq) => dq,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert query for document table");
            return Err(ApiError::DatabaseError("Failed to create insert query for document table".to_string()));
        }
    };

    let document_id: Uuid = match client
        .query_one(&document_query, &[&request.owner_id, &create_date, &title])
        .await
    {
        Ok(id) => id.get(0),
        Err(_) => {
            error!(target:"error_logger","Failed to insert document into document table");
            return Err(ApiError::DatabaseError(
                "Failed to insert into the documents table: {}".to_string(),
            ));
        }
    };

    let snapshot_query = match client.prepare("INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7)").await{
        Ok(sq) => sq,
        Err(_) => {
            error!(target:"error_logger","Failed to create INSERT query for document_snapshot table");
            return Err(ApiError::DatabaseError("Failed to create INSERT query for document_snapshot table".to_string()));
        }
    };

    let operation_query = match Client::prepare(&client,"INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)").await {
        Ok(oq) => oq,
        Err(_) => {
            error!(target: "error_logger","Failed to create INSERT query for operations table");
            return Err(ApiError::DatabaseError("Failed to create INSERT query for oeprations table".to_string()));
        }
    };

    let tx = match client.transaction().await {
        Ok(tx) => tx,
        Err(_) => {
            error!(target:"error_logger","Failed to start database transaction");
            return Err(ApiError::DatabaseError(
                "Failed to start transaction: {}".to_string(),
            ));
        }
    };

    // Execute the snapshot insert query
    match tx
        .execute(
            &snapshot_query,
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
    {
        Ok(_) => {
            info!(target:"request_logger","Successfull insert into the document_snapshot table");
        }
        Err(_) => {
            error!(target: "error_logger","Failed to insert into document_snapshot table");
            match tx.rollback().await {
                Ok(_) => {
                    info!(target:"request_logger","Successfully rolledback changes made to the database");
                }
                Err(_) => {
                    error!(target:"error_logger","Failed to rollback database changes");
                }
            }
            return Err(ApiError::DatabaseError(
                "Failed to insert into the document_snapshots table.".to_string(),
            ));
        }
    };

    let timestamp = chrono::Utc::now().to_rfc3339().to_string();

    match tx
        .execute(
            &operation_query,
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
    {
        Ok(_) => {
            info!(target:"request_logger","Successfully inserted row into operations table");
        }
        Err(_) => {
            error!(target:"error_logger","Failed to insert into operation table");
            match tx.rollback().await {
                Ok(_) => {
                    info!(target:"request_logger","Successfully rolledback changes made to the database");
                }
                Err(_) => {
                    error!(target:"error_logger","Failed to rollback database changes");
                }
            }
            return Err(ApiError::DatabaseError(
                "Failed to insert operation into the operations table: {}".to_string(),
            ));
        }
    }
    match tx.commit().await {
        Ok(_) => {
            info!(target:"requet_logger","Successfully commited database trasaction.");
        }
        Err(_) => {
            error!(target:"error_logger","Failed to commit database transaction");
            ApiError::DatabaseError("Failed to commit transaction".to_string());
        }
    };

    Ok(Json(CreateDocumentResponse {
        document_id,
        message: format!("Document {} created successuflly", document_id),
    }))
}

/// Fetch a document from the AWS RDB and initialize a RGA.
/// `id` is the document UUID.
#[get("/document/<id>")]
pub async fn fetch_document(
    id: String,
    rgas: &rocket::State<SharedRGAs>,
    replica_id: &rocket::State<Arc<Mutex<i64>>>,
    db: &rocket::State<Arc<Mutex<Client>>>,
) -> Result<(), ApiError> {
    let document_id: Uuid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return Err(ApiError::RequestFailed(
                "Failed to parse document id".to_string(),
            ));
        }
    };

    let mut rgas = rgas.lock().await;
    let client = db.lock().await;

    if rgas.contains_key(&document_id) {
        return Ok(());
    }

    let query = match client
        .prepare(
            "SELECT * from document_snapshots WHERE document_id=$1 ORDER BY ssn, sum, sid,seq;",
        )
        .await
    {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to prepare select query for document_snapshot table");
            return Err(ApiError::DatabaseError(
                "Failed to prepare select statement for document_snapshot table.".to_string(),
            ));
        }
    };

    let rows = match client.query(&query, &[&document_id]).await {
        Ok(r) => {
            info!(target:"request_logger","Successfull seelect statement for the document_snapshot table");
            r
        }
        Err(_) => {
            error!(target:"error_logger","Failed to execute select statement for the document_snapshot table");
            return Err(ApiError::DatabaseError(
                "Failed to find document in database".to_string(),
            ));
        }
    };

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

    Ok(())
}

/// Insert a value into the RGA of a specific document.
/* pub struct OperationRequest {
    value: Option<String>,
    s4vector: Option<S4Vector>,
    tombstone: bool,
    left: Option<S4Vector>,
    right: Option<S4Vector>,
}*/

/// Inserts a new value into the correcponding document's RGA.
///
/// Example Request:
/// {
///     "value" : "Some text here",
///     "s4vector: {
///                 "ssn": 2,
///                 "sum" : 4,
///                 "sid" : 3,
///                 "seq" : 3,
///                 },
///     "tombstone" : false,
///     "left" :  {
///                 "ssn": 2,
///                 "sum" : 4,
///                 "sid" : 3,
///                 "seq" : 3,
///               },
///     "right" : null
/// }
///
#[post("/document/<id>/insert", format = "json", data = "<request>")]
pub async fn insert(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
    db: &rocket::State<Arc<Mutex<Client>>>,
    sns_client: &rocket::State<Arc<Mutex<SnsClient>>>,
    topic: &rocket::State<Arc<Mutex<String>>>,
) -> Result<(), ApiError> {
    let document_id: Uuid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            error!(target:"error_logger","Failed to parse document id");
            return Err(ApiError::RequestFailed(
                "Failed to parse document id".to_string(),
            ));
        }
    };

    let mut rgas = rgas.lock().await;
    let mut client = db.lock().await;

    // Check if the document has been loaded
    let rga: &mut RGA = match rgas.get_mut(&document_id) {
        Some(r) => r,
        None => {
            error!(target:"error_logger","Document not found");
            return Err(ApiError::RequestFailed(String::from("Document not found")));
        }
    };

    let value: String = if request.value.is_some() {
        request.value.clone().unwrap()
    } else {
        error!(target:"error_logger","Value not found.");
        return Err(ApiError::RequestFailed("Value not found".to_string()));
    };

    let mut op: BroadcastOperation = match rga
        .local_insert(value.clone(), request.left, request.right, document_id)
        .await
    {
        Ok(obj) => obj,
        Err(_) => {
            error!(target:"error_logger","Failed to insert into file");
            return Err(ApiError::RequestFailed(
                "Error inserting into file".to_string(),
            ));
        }
    };

    op.document_id = document_id;

    let s4 = op.s4vector();

    let operation_query = match client.prepare("INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)").await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert query for operations table");
            return Err(ApiError::DatabaseError(
                "Failed to create insert query for operation table".to_string(),
            )); 
        }
    };

    let snapshot_query = match client.prepare("INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7)").await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert query for document_snapshot table");
            return Err(ApiError::DatabaseError(
                "Failed to create insert query for document_snapshot table".to_string(),
            )); 
        }
    };

    let current_time = chrono::Utc::now().to_rfc3339().to_string();

    let tx = match client.transaction().await {
        Ok(tx) => tx,
        Err(_) => {
            error!(target:"error_logger","Failed to create database transaction");
            return Err(ApiError::DatabaseError(
                "Failed to create database transaction".to_string(),
            ));
        }
    };

    match tx.execute(
        &operation_query,
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
    {
        Ok(_) => (),
        Err(_) => {
            return Err(ApiError::DatabaseError(
                "Failed to insert into operations table".to_string()
            ))
        }
    }

    match tx.execute(
        &snapshot_query,
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
    {
        Ok(_) => (),
        Err(_) => {
            return Err(ApiError::DatabaseError(
                "Failed to insert into document_snapshot table".to_string()
            ))
        }
    }

    //Broadcast to SNS
    match db::send_operation(Arc::clone(sns_client), &topic.lock().await, &op).await {
        Ok(_) => (),
        Err(_) => {
            error!(target:"error_logger","Failed to send SNS notification");
            return Err(ApiError::DatabaseError(format!(
                "Failed to send SNS notification"
            )))
        }
    };

    // After broadcast SNS to ensure it is sent
    match tx.commit().await {
        Ok(_) => (),
        Err(_) => {
            error!(target:"error_logger","Failed to commit database transaction");
            return Err(ApiError::DatabaseError(
                "Failed to commit database transaction".to_string()
            ))
        }
    }

    Ok(())
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
    let document_id: Uuid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            error!(target:"error_logger","Failed to parse document id");
            return Err(ApiError::RequestFailed("Failed to parse document id".to_string()));
        }
};

    let mut rgas = rgas.lock().await;
    let mut client = db.lock().await;

    // Check if the document has been loaded
    let rga: &mut RGA = match rgas.get_mut(&document_id) {
        Some(r) => r,
        None => {
            error!(target:"error_logger","Document not found");
            return Err(ApiError::RequestFailed("Document not found".to_string()));
        }
    };

    let value: String = if request.value.is_some() {
        request.value.clone().unwrap()
    } else {
        error!(target:"error_logger","Value not found");
        return Err(ApiError::RequestFailed("Value not found".to_string()));
    };

    let mut op: BroadcastOperation = match rga
        .local_update(request.s4vector.unwrap(), value.clone(), document_id)
        .await
    {
        Ok(obj) => obj,
        Err(_) => {
            error!(target:"error_logger","Failed to update file");
            return Err(ApiError::RequestFailed("Error updating file".to_string()));
        }
    };

    op.document_id = document_id;

    let s4 = op.s4vector();

    let operation_query = match client.prepare("INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)").await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert statement for operations table");
            return Err(ApiError::RequestFailed("Failed to create insert statement for operations table".to_string()));
        }
    };
    let snapshot_query = match client.prepare("INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (document_id,ssn,sum,sid,seq) DO UPDATE set value = EXCLUDED.value, tombstone = EXCLUDED.tombstone").await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert statement for document_snapshot table");
            return Err(ApiError::RequestFailed("Failed to create insert statement for document_snapshot table".to_string()));
        }
    };

    let current_time = chrono::Utc::now().to_rfc3339().to_string();

    let tx = match client.transaction().await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create database transaction");
            return Err(ApiError::RequestFailed("Failed to create database transaction".to_string()));
        }
    };

    match tx.execute(
        &operation_query,
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
    {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to run insert query for operations table");
            return Err(ApiError::RequestFailed("Failed to run insert query for operations table".to_string()));
        }
    };

    match tx.execute(
        &snapshot_query,
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
    {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to run insert query for document_snapshot table");
            return Err(ApiError::RequestFailed("Failed to run insert query for document_snapshot table".to_string()));
        }
    };

    match tx.commit().await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to commit database transaction");
            return Err(ApiError::RequestFailed("Failed to commit database transaction".to_string()));
        }
    };

    //Broadcast to SNS
    match db::send_operation(Arc::clone(sns_client), &topic.lock().await, &op).await {
        Ok(_) => (),
        Err(_) => {
            error!(target:"error_logger","Failed to send SNS notification");
            return Err(ApiError::DatabaseError(
                "Failed to send SNS notification".to_string()
            ));
        }
    };

    Ok(())
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
    let document_id: Uuid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            error!(target:"error_logger","Failed to parse document id");
            return Err(ApiError::RequestFailed("Failed to parse document id".to_string()));
        }
};

    let mut rgas = rgas.lock().await;
    let mut client = db.lock().await;

    // Check if the document has been loaded
    let rga: &mut RGA = match rgas.get_mut(&document_id) {
        Some(r) => r,
        None => 
        {
            error!(target:"error_logger","Document could not be found.");
            return Err(ApiError::RequestFailed(String::from("Document not found")));
        }
    };

    let mut op: BroadcastOperation = match rga
        .local_delete(request.s4vector.unwrap(), document_id)
        .await
    {
        Ok(obj) => obj,
        Err(_) => {
            error!(target:"error_logger","Failed to update file");
            return Err(ApiError::RequestFailed("Error updating file".to_string()));
        }
    };

    op.document_id = document_id;

    let s4 = op.s4vector();

    let operation_query = match client.prepare("INSERT INTO operations (document_id,ssn,sum,sid,seq,value,tombstone,timestamp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)").await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert query for operations table");
            return Err(ApiError::RequestFailed("Failed to create insert query for operations table".to_string()));
        }
    };
    let snapshot_query = match client.prepare("INSERT INTO document_snapshots (document_id,ssn,sum,sid,seq,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (document_id,ssn,sum,sid,seq) DO UPDATE set value = EXCLUDED.value, tombstone = EXCLUDED.tombstone").await {
        Ok(q) => q,
        Err(_) => {
            error!(target:"error_logger","Failed to create insert query for operations table");
            return Err(ApiError::RequestFailed("Failed to create insert query for operations table".to_string()));
        }
    };

    let current_time = chrono::Utc::now().to_rfc3339().to_string();

    let tx = match client.transaction().await {
        Ok(tx) => tx,
        Err(_) => {
            error!(target:"error_logger","Failed to create database transaction");
            return Err(ApiError::DatabaseError("Failed to create database transaction".to_string()));
        }
    };

    match tx.execute(
        &operation_query,
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
    .await{
        Ok(tx) => tx,
        Err(_) => {
            error!(target:"error_logger","Failed to perform insert into operations table");
            return Err(ApiError::DatabaseError("Failed to perform insert into operations table".to_string()));
        }
    };

    match tx.execute(
        &snapshot_query,
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
    .await {
        Ok(tx) => tx,
        Err(_) => {
            error!(target:"error_logger","Failed to perform insert into document_snapshot table");
            return Err(ApiError::DatabaseError("Failed to perform insert into document_snapshot table".to_string()));
        }
    };

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
