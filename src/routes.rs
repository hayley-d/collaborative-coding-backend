use std::sync::Arc;

use crate::rga::rga::RGA;
use crate::{ApiError, S4Vector};
use rocket::serde::json::Json;
use rocket::tokio::sync::Mutex;
use rocket::{get, post};
use serde::{Deserialize, Serialize};

type SharedRGA = Arc<Mutex<RGA>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct OperationRequest {
    operation: String,
    value: Option<String>,
    s4vector: Option<S4Vector>,
    left: Option<S4Vector>,
    right: Option<S4Vector>,
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
