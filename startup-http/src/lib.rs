#[macro_use]
extern crate tracing;

use std::net::AddrParseError;

use serde::{Deserialize, Serialize};
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

pub use error::{WebError, WebErrorExt};
pub use serve::serve_static;

mod error;
mod serve;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid listen address")]
    InvalidAddress(#[from] AddrParseError),

    #[error("invalid listen address")]
    Server(#[from] hyper::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpConfig {
    pub port: u16,
    pub address: String,
}

pub fn trace_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
}
