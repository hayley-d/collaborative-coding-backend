use crate::db;
use crate::rga::rga::RGA;
use crate::{ApiError, S4Vector};
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
type SharedDB = Arc<Mutex<db::Database>>;

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
    buffered_operations: u64,
    active_sessions: usize,
}

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
