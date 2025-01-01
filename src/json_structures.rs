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

/// Struct for holding the document snapshot data
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub document_id: Uuid,
    pub ssn: i64,
    pub sum: i64,
    pub sid: i64,
    pub seq: i64,
    pub value: String,
    pub tombstone: bool,
}

/// Represents the request body for operations.
#[derive(Debug, Serialize, Deserialize)]
pub struct OperationRequest {
    pub value: Option<String>,
    pub s4vector: Option<S4Vector>,
    pub tombstone: bool,
    pub left: Option<S4Vector>,
    pub right: Option<S4Vector>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SnsNotification {
    pub operation: String,
    pub message_id: String,
    pub topic_arn: String,
    pub message: String,
    pub timestamp: String,
}

/// BroadcastOpteration is the operation sent from one replica to another through AWS SNS
#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastOperation {
    /// Operations type (Insert,Update,Delete)
    pub operation: String,
    /// document id for the target document
    pub document_id: Uuid,
    /// Session number for s4vector
    pub ssn: i64,
    /// sum for s4vector
    pub sum: i64,
    /// Site ID or replica ID for the s4vector
    pub sid: i64,
    /// Sequence number for the s4vector
    pub seq: i64,
    pub value: Option<String>,
    pub left: Option<S4Vector>,
    pub right: Option<S4Vector>,
}

impl BroadcastOperation {
    /// Constructs the S4Vector for the broadcast operation
    pub fn s4vector(&self) -> S4Vector {
        return S4Vector {
            ssn: self.ssn as u64,
            sum: self.sum as u64,
            sid: self.sid as u64,
            seq: self.seq as u64,
        };
    }
}
