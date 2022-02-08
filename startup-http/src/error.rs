use std::fmt::{Debug, Write};

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use eyre::Report;
use serde::Serialize;

pub trait WebErrorExt<T> {
    fn with_status_code(self, code: StatusCode) -> Result<T, WebError>;
}

#[derive(Debug)]
pub enum WebError {
    Response(StatusCode, String),
    WithStatusCode(StatusCode, Report),
}

impl<T: Into<Report>> From<T> for WebError {
    fn from(err: T) -> Self {
        WebError::WithStatusCode(StatusCode::INTERNAL_SERVER_ERROR, err.into())
    }
}

impl<T, E: Into<Report>> WebErrorExt<T> for Result<T, E> {
    fn with_status_code(self, code: StatusCode) -> Result<T, WebError> {
        self.map_err(|err| WebError::WithStatusCode(code, err.into()))
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::Response(status, message) => {
                let response = ErrorResponse {
                    status: status.as_u16(),
                    message,
                };

                (status, Json(response)).into_response()
            }

            WebError::WithStatusCode(status, err) => {
                let mut message = String::new();

                let _ = write!(&mut message, "{:#}", err);

                if let Some(source) = err.source().and_then(|err| err.source()) {
                    write!(&mut message, "\nSource: {}", source).unwrap();
                }

                info!("{}", message);

                let response = ErrorResponse {
                    status: status.as_u16(),
                    message,
                };

                (status, Json(response)).into_response()
            }
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    status: u16,
    message: String,
}
