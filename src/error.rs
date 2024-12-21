use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ApiError {
    #[error("Dependency missing for the operation")]
    #[diagnostic(code(api::dependency_missing))]
    DependencyMissing,

    #[error("Invalid operation: {0}")]
    #[diagnostic(code(api::invalid_operation))]
    InvalidOperation(String),

    #[error("Failed to process request: {0}")]
    #[diagnostic(code(api::request_failed))]
    RequestFailed(String),
}
