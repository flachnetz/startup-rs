#[macro_use]
extern crate tracing;

use std::net::{AddrParseError, IpAddr, SocketAddr, ToSocketAddrs};

use serde::{Deserialize, Serialize};
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

pub use error::{WebError, WebErrorExt};
pub use serve::serve_static;

pub use crate::trace::ZipkinMakeSpan;
pub use crate::trace::{Layer as ZipkinTraceLayer};

mod error;
mod serve;
mod trace;

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpConfig {
    pub port: u16,
    pub address: String,
}

impl TryFrom<HttpConfig> for SocketAddr {
    type Error = AddrParseError;

    fn try_from(value: HttpConfig) -> Result<Self, Self::Error> {
        let ip: IpAddr = value.address.parse()?;
        Ok((ip, value.port).into())
    }
}

pub fn tracing_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, ZipkinMakeSpan> {
    TraceLayer::new_for_http()
        .make_span_with(trace::ZipkinMakeSpan::new())
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
}
