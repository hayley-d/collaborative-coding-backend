use std::sync::Arc;

use crate::rga::rga::RGA;
use crate::S4Vector;
use rocket::serde::json::Json;
use rocket::tokio::sync::Mutex;
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
pub async fn insert(request: Json<OperationRequest>, rga: &rocket::State<SharedRGA>) -> String {
    let mut rga = rga.lock().await.unwrap();

    if let Some(value) = &request.value {
        match rga.local_insert(value, request.left, request.right) {
            Ok(_) => "Insert successful".to_string(),
            Err(err) => format!("Insert Failed {}", err),
        }
    } else {
        "Insert failed: Missing value".to_string()
    }
}

/// Handles fontend-initiated update operation
#[post("/update", data = "<request>")]
pub async fn update(request: Json<OperationRequest>, rga: &rocket::State<SharedRGA>) -> String {
    let mut rga = rga.lock().await.unwrap();

    if let Some(value) = &request.value {
        match rga.local_update(request.s4vector.unwrap(), value) {
            Ok(_) => "Update successful".to_string(),
            Err(err) => format!("Update failed: {}", err),
        }
    } else {
        return "Update failed: Missing value".to_string();
    }
}

/// Handles fontend-initiated delete operation
#[post("/delete", data = "<request>")]
pub async fn delete(request: Json<OperationRequest>, rga: &rocket::State<SharedRGA>) -> String {
    let mut rga = rga.lock().await.unwrap();

    if let Some(value) = &request.value {
        match rga.local_update(request.s4vector.unwrap(), value) {
            Ok(_) => "Delete successful".to_string(),
            Err(err) => format!("Delete failed: {}", err),
        }
    } else {
        return "Delete failed: Missing value".to_string();
    }
}

/// Applies an insert opperation received from another replica
#[post("/remote/insert", data = "<request>")]
pub async fn insert(request: Json<OperationRequest>, rga: &rocket::State<SharedRGA>) -> String {
    let mut rga = rga.lock().await.unwrap();

    if let Some(value) = &request.value {
        match rga.remote_insert(value, request.left, request.right) {
            Ok(_) => "Insert successful".to_string(),
            Err(err) => format!("Insert Failed {}", err),
        }
    } else {
        "Insert failed: Missing value".to_string()
    }
}

/// Applies an update opperation received from another replica
#[post("/remote/update", data = "<request>")]
pub async fn update(request: Json<OperationRequest>, rga: &rocket::State<SharedRGA>) -> String {
    let mut rga = rga.lock().await.unwrap();

    if let Some(value) = &request.value {
        match rga.remote_update(request.s4vector.unwrap(), value) {
            Ok(_) => "Update successful".to_string(),
            Err(err) => format!("Update failed: {}", err),
        }
    } else {
        return "Update failed: Missing value".to_string();
    }
}

/// Applies a delete opperation received from another replica
#[post("/remote/delete", data = "<request>")]
pub async fn delete(request: Json<OperationRequest>, rga: &rocket::State<SharedRGA>) -> String {
    let mut rga = rga.lock().await.unwrap();

    if let Some(value) = &request.value {
        match rga.remote_update(request.s4vector.unwrap(), value) {
            Ok(_) => "Delete successful".to_string(),
            Err(err) => format!("Delete failed: {}", err),
        }
    } else {
        return "Delete failed: Missing value".to_string();
    }
}

/// Returns the current state of the RGA as a JSON object for frontend use.
#[get("/state")]
pub async fn state(rga: &rocket::State<SharedRGA>) -> Json<Vec<String>> {
    let rga = rga.lock().await.unwrap();

    return Json(rga.read());
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
