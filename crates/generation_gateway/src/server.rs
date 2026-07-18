//! axum HTTP surface for Protocol v1.1: submit, poll, providers catalog, behind a
//! constant-time bearer check.

use std::sync::Arc;

use axum::{
    extract::{Path, Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use crate::config::GatewayConfig;
use crate::jobs::JobStore;
use crate::protocol::{ErrorResponse, GenerateRequest, JobStatusResponse, SubmitResponse};
use crate::provider::ProviderKind;
use crate::registry::ProviderRegistry;
use crate::stub::StubProvider;

/// Shared, cloneable state for the axum router. The registry (immutable after
/// build) and the store are both `Arc` so handlers and the stub providers see the
/// same job records.
#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<ProviderRegistry>,
    pub store: Arc<JobStore>,
    pub config: Arc<GatewayConfig>,
}

/// Register the three phase-1 stub providers (one per kind) against a shared store.
pub fn build_stub_registry(store: &Arc<JobStore>) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    for kind in ProviderKind::ALL {
        registry.register(Arc::new(StubProvider::new(kind, store.clone())));
    }
    registry
}

/// Assemble app state for a stub gateway from a config: fresh store, stub
/// registry, and any per-kind default-provider overrides applied.
pub fn stub_app_state(config: GatewayConfig) -> AppState {
    let store = Arc::new(JobStore::new());
    let mut registry = build_stub_registry(&store);
    for (kind, name) in &config.default_providers {
        let _ = registry.set_default(*kind, name);
    }
    AppState {
        registry: Arc::new(registry),
        store,
        config: Arc::new(config),
    }
}

/// Build the router with bearer auth applied to every route.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/generate", post(generate))
        .route("/v1/jobs/{id}", get(job_status))
        .route("/v1/providers", get(providers))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ))
        .with_state(state)
}

async fn require_bearer(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if authorize(request.headers(), state.config.auth_token.as_deref()) {
        next.run(request).await
    } else {
        error_response(StatusCode::UNAUTHORIZED, "unauthorized".to_string())
    }
}

async fn generate(State(state): State<AppState>, Json(req): Json<GenerateRequest>) -> Response {
    match state.registry.route(req.kind, req.provider.as_deref()) {
        Ok(provider) => match provider.submit(&req) {
            Ok(job) => (
                StatusCode::OK,
                Json(SubmitResponse {
                    job_id: job.job_id,
                    status: "queued".to_string(),
                }),
            )
                .into_response(),
            Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn job_status(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some((kind, name)) = state.store.provider_of(&id) else {
        return error_response(StatusCode::NOT_FOUND, format!("unknown job: {id}"));
    };
    let provider = match state.registry.route(kind, Some(&name)) {
        Ok(provider) => provider,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };
    match provider.poll(&id) {
        Ok(status) => (StatusCode::OK, Json(JobStatusResponse::from_status(&status))).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

async fn providers(State(state): State<AppState>) -> Response {
    (StatusCode::OK, Json(state.registry.catalog())).into_response()
}

fn error_response(code: StatusCode, message: String) -> Response {
    (code, Json(ErrorResponse { error: message })).into_response()
}

/// True when the request is authorized: no token configured → always; otherwise a
/// matching `Authorization: Bearer <token>` (constant-time comparison).
pub fn authorize(headers: &HeaderMap, expected: Option<&str>) -> bool {
    match expected {
        None => true,
        Some(token) => bearer_token(headers)
            .map(|presented| ct_eq(presented.as_bytes(), token.as_bytes()))
            .unwrap_or(false),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?;
    let token = parts.next()?.trim();
    if scheme.eq_ignore_ascii_case("bearer") {
        Some(token.to_string())
    } else {
        None
    }
}

/// Constant-time byte comparison (beyond the unavoidable length check).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_auth(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, value.parse().unwrap());
        headers
    }

    #[test]
    fn no_token_configured_allows_everything() {
        assert!(authorize(&HeaderMap::new(), None));
        assert!(authorize(&headers_with_auth("Bearer whatever"), None));
    }

    #[test]
    fn correct_bearer_is_authorized() {
        assert!(authorize(&headers_with_auth("Bearer secret"), Some("secret")));
        // Scheme is case-insensitive.
        assert!(authorize(&headers_with_auth("bearer secret"), Some("secret")));
    }

    #[test]
    fn wrong_or_missing_token_is_rejected() {
        assert!(!authorize(&headers_with_auth("Bearer nope"), Some("secret")));
        assert!(!authorize(&HeaderMap::new(), Some("secret")));
        // Wrong scheme.
        assert!(!authorize(&headers_with_auth("Basic secret"), Some("secret")));
    }

    #[test]
    fn ct_eq_matches_only_equal_bytes() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }

    #[test]
    fn stub_app_state_registers_all_kinds() {
        let state = stub_app_state(GatewayConfig::default());
        for kind in ProviderKind::ALL {
            assert!(state.registry.route(kind, None).is_ok());
        }
    }
}
