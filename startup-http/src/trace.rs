use std::fmt::Debug;
use std::{borrow::Cow, error::Error as StdError, future::Future, pin::Pin, task::Poll};

use axum::body::BoxBody;
use axum::http::Request;
use futures_util::FutureExt as f_FutureExt;
use hyper::body::{Bytes, HttpBody};
use hyper::{header, HeaderMap, Method, Response, Version};
use opentelemetry::trace::FutureExt;
use opentelemetry::trace::{SpanKind, StatusCode, TraceContextExt, Tracer};
use opentelemetry::Context;
use opentelemetry_http::HeaderExtractor;
use opentelemetry_semantic_conventions::trace::{
    HTTP_FLAVOR, HTTP_METHOD, HTTP_STATUS_CODE, HTTP_TARGET, HTTP_URL, HTTP_USER_AGENT,
};
use tower_http::trace::{DefaultMakeSpan, MakeSpan};
use tracing::{Level, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Clone)]
pub(crate) struct ZipkinMakeSpan {
    delegate: DefaultMakeSpan,
}

impl ZipkinMakeSpan {
    pub fn new() -> Self {
        Self {
            delegate: DefaultMakeSpan::new().level(Level::INFO),
        }
    }
}

impl<B> MakeSpan<B> for ZipkinMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let span = self.delegate.make_span(request);

        // connects the tracing::Span with the opentelemetry::Span.
        let otel_context = Context::current();
        let otel_span = otel_context.span();
        let otel_span_context = otel_span.span_context();

        if otel_span_context.is_remote() {
            span.add_link(otel_span_context.clone());
        }

        span
    }
}

/// [`Layer`] that adds high level [opentelemetry propagation] to a [`Service`].
///
/// [`Layer`]: tower_layer::Layer
/// [opentelemetry propagation]: https://opentelemetry.io/docs/java/manual_instrumentation/#context-propagation
/// [`Service`]: tower_service::Service
#[derive(Debug, Copy, Clone, Default)]
pub struct Layer {}

impl Layer {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl<S> tower_layer::Layer<S> for Layer
where
    S: Clone,
{
    type Service = Service<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Service::new(inner)
    }
}

/// Middleware [`Service`] that propagates the opentelemetry trace header, configures a span for
/// the request, and records any exceptions.
///
/// [`Service`]: tower_service::Service
#[derive(Clone)]
pub struct Service<S: Clone> {
    inner: S,
}

impl<S> Service<S>
where
    S: Clone,
{
    fn new(inner: S) -> Self {
        Self { inner }
    }
}

type CF<R, E> = dyn Future<Output = Result<R, E>> + Send;

impl<B, S> tower_service::Service<Request<B>> for Service<S>
where
    S: tower_service::Service<
        Request<B>,
        Response = Response<http_body::combinators::UnsyncBoxBody<Bytes, axum::Error>>,
    >,
    S::Future: 'static + Send,
    B: 'static,
    S::Error: std::fmt::Debug + StdError,
    S: Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<CF<Self::Response, Self::Error>>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let tracer = opentelemetry::global::tracer("foo");

        let parent_context = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(req.headers()))
        });

        let uri = req.uri();

        let mut builder = tracer.span_builder(uri.path().to_string()).with_kind(SpanKind::Server);

        let mut attributes = Vec::with_capacity(11);
        attributes.push(HTTP_METHOD.string(http_method_str(req.method())));
        attributes.push(HTTP_FLAVOR.string(http_flavor(req.version())));
        attributes.push(HTTP_URL.string(uri.to_string()));

        if let Some(path) = uri.path_and_query() {
            attributes.push(HTTP_TARGET.string(path.as_str().to_string()));
        }
        if let Some(user_agent) = req.headers().get(header::USER_AGENT).and_then(|s| s.to_str().ok()) {
            attributes.push(HTTP_USER_AGENT.string(user_agent.to_string()));
        }

        builder.attributes = Some(attributes);

        let span = tracer.build_with_context(builder, &parent_context);
        let cx = Context::current_with_span(span);
        let attachment = cx.clone().attach();

        let x = self.inner.call(req);

        let fut = x.with_context(cx.clone()).map(
            move |res: Result<Response<BoxBody>, <S as tower_service::Service<Request<B>>>::Error>| match res {
                Ok(ok_res) => {
                    let span = cx.span();
                    span.set_attribute(HTTP_STATUS_CODE.i64(i64::from(ok_res.status().as_u16())));
                    if ok_res.status().is_server_error() {
                        span.set_status(
                            StatusCode::Error,
                            ok_res
                                .status()
                                .canonical_reason()
                                .map(ToString::to_string)
                                .unwrap_or_default(),
                        );
                    };

                    // the body future will be polled after handling the request,
                    // so we need to pass the span to it and only end it once everything is polled
                    // to finish.
                    let mapped = ok_res.map(|body| EndSpanBody { body, span_context: cx }.boxed_unsync());

                    Ok(mapped)
                }
                Err(error) => {
                    let span = cx.span();
                    span.set_status(StatusCode::Error, format!("{:?}", error));
                    span.record_exception(&error);
                    span.end();
                    Err(error)
                }
            },
        );

        drop(attachment);
        Box::pin(fut)
    }
}

fn http_method_str(method: &Method) -> Cow<'static, str> {
    match method {
        &Method::OPTIONS => "OPTIONS".into(),
        &Method::GET => "GET".into(),
        &Method::POST => "POST".into(),
        &Method::PUT => "PUT".into(),
        &Method::DELETE => "DELETE".into(),
        &Method::HEAD => "HEAD".into(),
        &Method::TRACE => "TRACE".into(),
        &Method::CONNECT => "CONNECT".into(),
        &Method::PATCH => "PATCH".into(),
        other => other.to_string().into(),
    }
}

fn http_flavor(version: Version) -> Cow<'static, str> {
    match version {
        Version::HTTP_09 => "0.9".into(),
        Version::HTTP_10 => "1.0".into(),
        Version::HTTP_11 => "1.1".into(),
        Version::HTTP_2 => "2.0".into(),
        Version::HTTP_3 => "3.0".into(),
        other => format!("{:?}", other).into(),
    }
}

#[pin_project::pin_project]
struct EndSpanBody {
    #[pin]
    body: BoxBody,
    span_context: Context,
}

impl HttpBody for EndSpanBody {
    type Data = <BoxBody as HttpBody>::Data;
    type Error = <BoxBody as HttpBody>::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let this = self.project();

        let result = futures_util::ready!(this.body.poll_data(cx));
        if result.is_some() {
            // the caller is not expected to call `poll_trailers` if this function
            // does not return None. So we need to end the span here.
            this.span_context.span().end();
        }

        Poll::Ready(result)
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        let this = self.project();

        let result = futures_util::ready!(this.body.poll_trailers(cx));

        // this is it, we've finished the request, close the span.
        this.span_context.span().end();

        Poll::Ready(result)
    }
}
