#[macro_use]
extern crate tracing;

use std::net::SocketAddr;
use std::net::{AddrParseError, IpAddr};
use std::str::FromStr;

use axum::body::Body;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

pub use error::{WebError, WebErrorExt};
pub use serve::serve_static;

mod error;
mod serve;
mod trace;

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

pub async fn launch(config: &HttpConfig, router: Router<Body>) -> Result<(), Error> {
    let router = router
        // add ping routes
        .route("/ping", get(handle_ping))
        .route("/admin/ping", get(handle_ping))
        .layer(CompressionLayer::new().no_br())
        // add logging + tracing
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::ZipkinMakeSpan::new())
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(trace::Layer::new());

    let ip = IpAddr::from_str(&config.address)?;
    let addr = SocketAddr::from((ip, config.port));

    info!("Open http server on {}", addr);
    axum::Server::bind(&addr).serve(router.into_make_service()).await?;

    info!("Server has stopped");

    Ok(())
}

async fn handle_ping() -> impl IntoResponse {
    "pong"
}
