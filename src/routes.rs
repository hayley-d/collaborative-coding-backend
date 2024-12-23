use crate::rga::rga::RGA;
use crate::{ApiError, CreateDocumentRequest, CreateDocumentResponse, OperationRequest, S4Vector};
use rocket::serde::json::Json;
use rocket::tokio::sync::Mutex;
use rocket::{get, post};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_postgres::Client;
use uuid::Uuid;

/// This module implements routes for managing collaborative documents
/// using RGAs (Replicated Growable Arrays) in a distributed system.
/// It includes functionality to fetch documents, synchronize with the
/// database, manage CRDT operations, and monitor replica health.

/// Shared state type: Maps document IDs to their corresponding RGA instances.
type SharedRGAs = Arc<Mutex<HashMap<String, RGA>>>;

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
#[post("/create", format = "json", data = "<request>")]
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

    // Serialize S4Vector to JSON string
    let initial_s4vector =
        serde_json::json!({"ssn":0,"sum":0,"sid":replica_id,"seq":0}).to_string();

    // Start Database trasaction to ensure atomicity
    let tx = client.transaction().await.map_err(|e| {
        ApiError::DatabaseError(format!("Failed to start transaction: {}", e.to_string()))
    })?;

    // SQL query to insert a new snapshot into the document_snapshots table
    let snapshot_query = r#"INSERT INTO document_snapshots (document_id,s4vector,value,tombstone) VALUES ($1,$2,$3,$4)"#;
    // SQL query to insert a new operation into the operations table
    let operation_query = r#"INSERT INTO operations (replica_id,document_id,datetime,operation,s4vector,value,tombstone) VALUES ($1,$2,$3,$4,$5,$6,$7)"#;

    // Execute the snapshot insert query
    tx.execute(
        snapshot_query,
        &[&document_id, &initial_s4vector, &initial_content, &false],
    )
    .await
    .map_err(|e| {
        ApiError::DatabaseError(format!(
            "Failed to insert document snapshot into the document_snapshots table: {}",
            e.to_string()
        ))
    })?;

    // Execute the operation insert query
    tx.execute(
        operation_query,
        &[
            &replica_id,
            &document_id,
            &create_date,
            &"Insert",
            &initial_s4vector,
            &Some(initial_content.clone()),
            &false,
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

/*
/// Fetch a document from the Aurora DB and initialize an RGA.
#[get("/document/<id>")]
async fn fetch_document(
    id: String,
    rgas: &rocket::State<SharedRGAs>,
    db: &rocket::State<SharedDB>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    if rgas.contains_key(&id) {
        return Ok(());
    }

    let operations = db
        .lock()
        .await
        .fetch_document(&id)
        .map_err(|e| ApiError::DatabaseError(e))?;
    let rga = RGA::create_from(operations, 1, 1); // Session ID and Site ID placeholders.
    rgas.insert(id.clone(), rga);

    return Ok(());
}

/// Insert a value into the RGA of a specific document.
#[post("/document/<id>/insert", data = "<request>")]
pub async fn insert(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga: &mut RGA = match rgas.get_mut(&id) {
        Some(r) => r,
        None => return Err(ApiError::RequestFailed(String::from("Document not found"))),
    };

    if let Some(value) = &request.value {
        rga.local_insert(value.clone(), request.left, request.right)
            .await
            .map_err(|_| ApiError::DependencyMissing)?;
        Ok(())
    } else {
        Err(ApiError::InvalidOperation(
            "Value is required for insert".to_string(),
        ))
    }
}

/// Update a value in the RGA of a specific document.
#[post("/document/<id>/update", data = "<request>")]
pub async fn update(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get_mut(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document not found")))?;

    if let Some(value) = &request.value {
        rga.local_update(
            request
                .left
                .ok_or_else(|| ApiError::InvalidOperation("Left neighbor required".into()))?,
            value.clone(),
        )
        .await
        .map_err(|_| ApiError::DependencyMissing)?;
        Ok(())
    } else {
        Err(ApiError::InvalidOperation(
            "Value is required for update".to_string(),
        ))
    }
}

/// Delete a value from the RGA of a specific document.
#[post("/document/<id>/delete", data = "<request>")]
pub async fn delete(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get_mut(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document not found")))?;

    rga.local_delete(
        request
            .left
            .ok_or_else(|| ApiError::InvalidOperation("Left neighbor required".into()))?,
    )
    .await
    .map_err(|_| ApiError::DependencyMissing)?;
    Ok(())
}

/// Applies an insert opperation received from another replica
#[post("/document/<id>/remote/insert", data = "<request>")]
pub async fn remote_insert(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get_mut(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document not found")))?;

    if let Some(value) = &request.value {
        rga.remote_insert(
            value.to_string(),
            request.s4vector.unwrap(),
            request.left,
            request.right,
        )
        .await;
        return Ok(());
    } else {
        Err(ApiError::InvalidOperation(
            "Value is required for insert".to_string(),
        ))
    }
}

/// Applies an update opperation received from another replica
#[post("/document/<id>/remote/update", data = "<request>")]
pub async fn remote_update(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get_mut(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document not found")))?;

    if let Some(value) = &request.value {
        rga.remote_update(request.s4vector.unwrap(), value.to_string())
            .await;
        return Ok(());
    } else {
        Err(ApiError::InvalidOperation(
            "Value is required for insert".to_string(),
        ))
    }
}

/// Applies a delete opperation received from another replica
#[post("/document/<id>/remote/delete", data = "<request>")]
pub async fn remote_delete(
    id: String,
    request: Json<OperationRequest>,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get_mut(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document not found")))?;

    rga.remote_delete(request.s4vector.unwrap()).await;
    return Ok(());
}

/// Returns the current state of the RGA as a JSON object for frontend use.
#[get("/document/<id>/state")]
pub async fn state(
    id: String,
    rgas: &rocket::State<SharedRGAs>,
) -> Result<Json<Vec<String>>, ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get_mut(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document not found")))?;

    return Ok(Json(rga.read().await));
}


*/
/*async fn broadcast_operation(
    url: &str,
    operation: &OperationRequest,
) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client.post(url).json(&operation).send().await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(reqwest::Error::new(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "Broadcast failed",
        ))
    }
}*/
