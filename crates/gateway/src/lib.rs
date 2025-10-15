mod gateway;
mod security;
mod state;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    http::{header, HeaderName, HeaderValue, Method}, routing::{any, get}, Json, Router
};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{
    gateway::{handler_gateway, handler_websocket, GatewaytHandler},
    security::middleware::security_headers_middleware, state::AppState,
};

pub const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
pub const REAL_IP_HEADER: &str = "x-real-ip";


async fn api_health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn api_versions() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "versions": {
            "v1": {
                "status": "stable",
                "base_url": "/api/v1",
                "documentation": "/docs/v1"
            }
        }
    }))
}

pub async fn start() {
    utils::setup_env();
    
    let state = Arc::new(AppState::new().await);

    let trace_layer = tower_http::trace::TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<_>| {
            tracing::info_span!(
                "request",
                method = %request.method(),
                uri = %request.uri(),
                trace_id = %utils::xid::new(),
            )
        })
        .on_response(
            |response: &axum::http::Response<_>,
             latency: std::time::Duration,
             _span: &tracing::Span| {
                tracing::debug!(
                    status = %response.status(),
                    latency = ?latency,
                    "response"
                );
            },
        );

    let origins = utils::vars::get_allow_origins();
    let cors_layer = CorsLayer::new()
            .allow_origin(if origins.contains(&"*".to_string()) {
                AllowOrigin::any()
            } else {
                let origins = origins.clone();
                AllowOrigin::predicate(move |origin: &HeaderValue, _| {
                    origins.contains(
                        &String::from_utf8(origin.as_bytes().to_vec()).unwrap_or("".to_string()),
                    )
                })
            })
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_credentials(!origins.contains(&"*".to_string()))
            .allow_headers([
                header::AUTHORIZATION,
                header::ACCEPT,
                header::CONTENT_TYPE,
                header::UPGRADE,
                header::HOST,
                header::CONNECTION,
                header::ORIGIN,
                header::SEC_WEBSOCKET_KEY,
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderName::from_static(REAL_IP_HEADER),
                HeaderName::from_static(FORWARDED_FOR_HEADER),
            ]);       

    // start cluster node
    let node = {
        let state = state.clone();
        Arc::new(cluster::Node::new(state, GatewaytHandler).await)
    };

    let app = Router::new()
        // Redirect root path to latest version docs or return version info
        .route("/health", any(api_health_check))
        .route("/ws", any(handler_websocket))
        .route("/{service}/{version}/{*params}", any(handler_gateway))
        .route("/", get(api_versions))
        .with_state(node)
        .layer(trace_layer)
        .layer(cors_layer)
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(tower_http::catch_panic::CatchPanicLayer::new());

    let listener = tokio::net::TcpListener::bind(&utils::vars::get_server_bind())
        .await
        .unwrap();

    let graceful = axum::serve(
            listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(utils::shutdown_signal());
    
    if let Err(e) = graceful.await {
        tracing::error!("{}:{} server error: {:?}", file!(), line!(), e);
    }
}
