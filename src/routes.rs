use crate::rga::rga::RGA;
use crate::{db, ApiError, S4Vector};
use rocket::serde::json::Json;
use rocket::tokio::sync::Mutex;
use rocket::{get, post};
use serde::{Deserialize, Serialize};
/// This module implements routes for managing collaborative documents
/// using RGAs (Replicated Growable Arrays) in a distributed system.
/// It includes functionality to fetch documents, synchronize with the
/// database, manage CRDT operations, and monitor replica health.
use std::collections::HashMap;
use std::sync::Arc;

/// Shared state type: Maps document IDs to their corresponding RGA instances.
type SharedRGAs = Arc<Mutex<HashMap<String, RGA>>>;

/// Represents the request body for operations.
#[derive(Debug, Serialize, Deserialize)]
pub struct OperationRequest {
    operation: String,
    value: Option<String>,
    s4vector: Option<S4Vector>,
    tombstone: bool,
    left: Option<S4Vector>,
    right: Option<S4Vector>,
}

/// Represents the health response structure.
#[derive(Debug, Serialize, Deserialize)]
struct HealthResponse {
    uptime: String,
    buffered_operations: usize,
    active_sessions: usize,
}

/// Fetch a document from the Aurora DB and initialize an RGA.
#[get("/document/<id>")]
async fn fetch_document(id: String, rgas: &rocket::State<SharedRGAs>) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    if rgas.contains_key(&id) {
        return Ok(());
    }

    let operations = db::fetch_document(&id).map_err(|e| ApiError::DatabaseError(e))?;
    let rga = RGA::create_from(operations, 1, 1); // Session ID and Site ID placeholders.
    rgas.insert(id.clone(), rga);

    return Ok(());
}

/// Synchronize the in-memory RGA with the Aurora DB version.
#[post("/sync/<id>")]
async fn sync_document(id: String, rgas: &rocket::State<SharedRGAs>) -> Result<(), ApiError> {
    let mut rgas = rgas.lock().await;
    let rga = rgas
        .get(&id)
        .ok_or_else(|| ApiError::RequestFailed(String::from("Document does not exist")))?;

    let operations = rga.get_all_operations();
    db::sync_document(&id, operations).map_err(|e| ApiError::DatabaseError(e))?;

    return Ok(());
}

/// Handles fontend-initiated insert operation
#[post("/insert", data = "<request>")]
pub async fn insert(
    request: Json<OperationRequest>,
    rga: &rocket::State<SharedRGA>,
) -> Result<(), ApiError> {
    let mut rga = rga.lock().await;

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

/// Handles fontend-initiated update operation
#[post("/update", data = "<request>")]
pub async fn update(
    request: Json<OperationRequest>,
    rga: &rocket::State<SharedRGA>,
) -> Result<(), ApiError> {
    let mut rga = rga.lock().await;

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

/// Handles fontend-initiated delete operation
#[post("/delete", data = "<request>")]
pub async fn delete(
    request: Json<OperationRequest>,
    rga: &rocket::State<SharedRGA>,
) -> Result<(), ApiError> {
    let mut rga = rga.lock().await;
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
#[post("/remote/insert", data = "<request>")]
pub async fn remote_insert(
    request: Json<OperationRequest>,
    rga: &rocket::State<SharedRGA>,
) -> Result<(), ApiError> {
    let mut rga = rga.lock().await;

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
#[post("/remote/update", data = "<request>")]
pub async fn remote_update(
    request: Json<OperationRequest>,
    rga: &rocket::State<SharedRGA>,
) -> Result<(), ApiError> {
    let mut rga = rga.lock().await;

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
#[post("/remote/delete", data = "<request>")]
pub async fn remote_delete(
    request: Json<OperationRequest>,
    rga: &rocket::State<SharedRGA>,
) -> Result<(), ApiError> {
    let mut rga = rga.lock().await;

    rga.remote_delete(request.s4vector.unwrap()).await;
    return Ok(());
}

/// Returns the current state of the RGA as a JSON object for frontend use.
#[get("/state")]
pub async fn state(rga: &rocket::State<SharedRGA>) -> Result<Json<Vec<String>>, ApiError> {
    let rga = rga.lock().await;

    return Ok(Json(rga.read().await));
}

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
