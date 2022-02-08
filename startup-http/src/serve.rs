use axum::error_handling::HandleErrorLayer;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get_service, MethodRouter};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

/// Serves files from the given directory as static files.
/// This method will aso configure caching headers to cache the files forever.
///
/// Use like this: `.nest("/public", serve_static("./files/pub"))`
///
pub fn serve_static(path: impl AsRef<std::path::Path>) -> MethodRouter {
    let add_cache_control = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=604800, immutable"),
    );

    get_service(ServeDir::new(path).precompressed_gzip())
        .layer(add_cache_control)
        .layer(HandleErrorLayer::new(io_error_to_response))
}

async fn io_error_to_response(error: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to serve static file: {}", error),
    )
}
