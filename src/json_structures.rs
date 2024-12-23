use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::S4Vector;

/// Request body for creating a new document.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    pub owner_id: Uuid,
    pub title: String,
}

/// Response Body for the result of creating a new document
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDocumentResponse {
    pub document_id: Uuid, // Auto-generated document id
    pub message: String,   // Confirmation message
}

/// Response structure for a fetched document
#[derive(Debug, Serialize, Deserialize)]
pub struct FetchDocumentResponse {
    pub document_id: Uuid,
    pub title: String,
    pub owner_id: Uuid,
    pub creation_date: String,
    pub operations: Vec<Operation>,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    document_id: u64,
    s4vector: S4Vector,
    value: String,
    tombstone: bool,
    left: S4Vector,
    right: S4Vector,
}
