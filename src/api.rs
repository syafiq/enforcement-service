//! HTTP API for the enforcement service

use crate::service::{EnforcementService, Session};
use crate::error::Result;
use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// API state
pub struct ApiState {
    pub service: Arc<EnforcementService>,
}

/// Create access request
#[derive(Debug, Deserialize)]
pub struct CreateAccessRequest {
    pub entity_id: String,
    pub session_id: Option<Uuid>,
}

/// Create access response
#[derive(Debug, Serialize)]
pub struct CreateAccessResponse {
    pub session_id: Uuid,
    pub granted_capabilities: Vec<String>,
    pub expires_at: String,
}

/// Execute operation request
#[derive(Debug, Deserialize)]
pub struct ExecuteOperationRequest {
    pub session_id: Uuid,
    pub interface: String,
    pub operation: String,
    pub parameters: serde_json::Value,
}

/// Execute operation response
#[derive(Debug, Serialize)]
pub struct ExecuteOperationResponse {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Audit log query parameters
#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub entity_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize { 100 }

/// Create the API router
pub fn create_router(service: Arc<EnforcementService>) -> Router {
    let state = Arc::new(ApiState { service });
    
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/hal/access", post(create_hal_access))
        .route("/api/v1/hal/capabilities", get(get_capabilities))
        .route("/api/v1/entities", get(list_entities))
        .route("/api/v1/audit", get(get_audit_log))
        .route("/api/v1/stats", get(get_stats))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "hal-enforcement-service",
        "version": crate::VERSION,
    }))
}

/// Create HAL access session
async fn create_hal_access(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateAccessRequest>,
) -> Result<Json<CreateAccessResponse>, AppError> {
    let session = state.service.create_session(&req.entity_id).await?;
    
    Ok(Json(CreateAccessResponse {
        session_id: session.session_id,
        granted_capabilities: session.granted_capabilities,
        expires_at: session.expires_at.to_rfc3339(),
    }))
}

/// Get capabilities for an entity
async fn get_capabilities(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entity_id = params.get("entity_id")
        .ok_or_else(|| crate::error::EnforcementError::Config("entity_id required".to_string()))?;
    
    let config = state.service.get_entity_config(entity_id)
        .ok_or_else(|| crate::error::EnforcementError::EntityNotFound(entity_id.clone()))?;
    
    Ok(Json(serde_json::json!({
        "entity_id": entity_id,
        "capabilities": config.capabilities.list_granted(),
        "rate_limits": config.rate_limits,
        "quotas": config.quotas,
    })))
}

/// List all entities
async fn list_entities(
    State(state): State<Arc<ApiState>>,
) -> Json<serde_json::Value> {
    let entities = state.service.list_entities();
    
    Json(serde_json::json!({
        "entities": entities,
        "count": entities.len(),
    }))
}

/// Get audit log
async fn get_audit_log(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AuditLogQuery>,
) -> Json<serde_json::Value> {
    let events = state.service.get_audit_log(
        query.entity_id.as_deref(),
        query.limit,
    ).await;
    
    let events_json: Vec<_> = events.iter().map(|e| {
        serde_json::json!({
            "timestamp": e.timestamp.to_rfc3339(),
            "entity_id": e.entity_id,
            "session_id": e.session_id,
            "interface": e.interface,
            "operation": e.operation,
            "success": e.success,
            "error": e.error,
        })
    }).collect();
    
    Json(serde_json::json!({
        "events": events_json,
        "count": events_json.len(),
    }))
}

/// Get service statistics
async fn get_stats(
    State(state): State<Arc<ApiState>>,
) -> Json<serde_json::Value> {
    let active_sessions = state.service.active_sessions_count().await;
    let total_entities = state.service.list_entities().len();
    
    Json(serde_json::json!({
        "active_sessions": active_sessions,
        "total_entities": total_entities,
        "version": crate::VERSION,
    }))
}

/// Error wrapper for API responses
struct AppError(crate::error::EnforcementError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            crate::error::EnforcementError::EntityNotFound(ref msg) => {
                (StatusCode::NOT_FOUND, msg.clone())
            }
            crate::error::EnforcementError::CapabilityDenied { ref entity, ref capability } => {
                (StatusCode::FORBIDDEN, format!("{} denied for {}", capability, entity))
            }
            crate::error::EnforcementError::RateLimitExceeded { ref entity, ref message } => {
                (StatusCode::TOO_MANY_REQUESTS, format!("{}: {}", entity, message))
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
        };
        
        let body = Json(serde_json::json!({
            "error": message,
        }));
        
        (status, body).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<crate::error::EnforcementError>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
