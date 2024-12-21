use miette::Diagnostic;
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::{Request, Response};
use std::io::Cursor;
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

impl<'r> Responder<'r, 'static> for ApiError {
    fn respond_to(self, _: &'r Request<'_>) -> Result<Response<'static>, Status> {
        let message = format!("{:?}", self);
        let status = match self {
            ApiError::DependencyMissing => Status::Ok,
            ApiError::InvalidOperation(_) => Status::BadRequest,
            ApiError::RequestFailed(_) => Status::InternalServerError,
        };

        return Response::build()
            .status(status)
            .header(ContentType::Plain)
            .sized_body(message.len(), Cursor::new(message))
            .ok();
    }
}
