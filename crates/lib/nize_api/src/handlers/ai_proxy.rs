// @zen-component: PLAN-028-AiProxy
//
//! AI proxy handler — routes AI SDK requests through Rust with injected auth headers.
//!
//! Single endpoint `POST /ai-proxy` that:
//! 1. Authenticates the user (JWT cookie)
//! 2. Reads target URL and provider type from query params
//! 3. Decrypts the user's API key for that provider from config
//! 4. Injects the provider-specific auth header
//! 5. Proxies the request and streams the response back

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::error::AppError;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::config;

/// Query parameters for the AI proxy endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct AiProxyQuery {
    /// Target URL to proxy the request to.
    pub target: String,
    /// Provider type: "anthropic", "openai", or "google".
    pub provider: String,
}

/// Provider type → config key + auth header mapping.
struct ProviderMapping {
    config_key: &'static str,
    env_fallback: &'static str,
    auth_header_name: &'static str,
    auth_header_prefix: &'static str,
}

fn get_provider_mapping(provider: &str) -> Option<ProviderMapping> {
    match provider {
        "anthropic" => Some(ProviderMapping {
            config_key: "agent.apiKey.anthropic",
            env_fallback: "ANTHROPIC_API_KEY",
            auth_header_name: "x-api-key",
            auth_header_prefix: "",
        }),
        "openai" => Some(ProviderMapping {
            config_key: "agent.apiKey.openai",
            env_fallback: "OPENAI_API_KEY",
            auth_header_name: "authorization",
            auth_header_prefix: "Bearer ",
        }),
        "google" => Some(ProviderMapping {
            config_key: "agent.apiKey.google",
            env_fallback: "GOOGLE_GENERATIVE_AI_API_KEY",
            auth_header_name: "x-goog-api-key",
            auth_header_prefix: "",
        }),
        _ => None,
    }
}

/// `POST /ai-proxy` — proxy AI SDK requests with injected auth headers.
// @zen-impl: PLAN-028-1.4
pub async fn ai_proxy_handler(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthenticatedUser>,
    Query(params): Query<AiProxyQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, AppError> {
    // Validate provider type
    let mapping = get_provider_mapping(&params.provider).ok_or_else(|| {
        AppError::Forbidden(format!("Unknown provider type: {}", params.provider))
    })?;

    // Validate target URL
    let target_url: url::Url = params
        .target
        .parse()
        .map_err(|_| AppError::Validation("Invalid target URL".into()))?;

    // Ensure target is HTTPS (or localhost for dev)
    let host = target_url.host_str().unwrap_or("");
    let is_safe = target_url.scheme() == "https"
        || host == "localhost"
        || host == "127.0.0.1"
        || host == "::1";
    if !is_safe {
        return Err(AppError::Validation(
            "Target URL must use HTTPS or localhost".into(),
        ));
    }

    // Decrypt the API key for this provider
    let api_key = config::decrypt_secret_config_value(
        &state.pool,
        &state.config_cache,
        &user.0.sub,
        mapping.config_key,
        &state.config.mcp_encryption_key,
        Some(mapping.env_fallback),
    )
    .await?
    .ok_or_else(|| {
        AppError::Validation(format!(
            "No API key configured for provider: {}",
            params.provider
        ))
    })?;

    // Build the outbound request
    let client = reqwest::Client::new();
    let mut req_builder = client.post(target_url.as_str());

    // Forward relevant headers (content-type, accept, etc.)
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        // Forward content-type, accept, and provider-specific headers
        // Skip host, cookie, connection, and auth headers (we inject our own)
        if matches!(
            name_str.as_str(),
            "content-type" | "accept" | "anthropic-version" | "anthropic-beta"
        ) && let Ok(v) = value.to_str()
        {
            req_builder = req_builder.header(name.as_str(), v);
        }
    }

    // Inject the provider-specific auth header
    let auth_value = format!("{}{}", mapping.auth_header_prefix, api_key);
    req_builder = req_builder.header(mapping.auth_header_name, &auth_value);

    // Stream the request body
    let body_bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read request body: {e}")))?;
    req_builder = req_builder.body(body_bytes);

    // Execute the upstream request
    let upstream_response = req_builder
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Upstream request failed: {e}")))?;

    // Build the response, streaming the body back
    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut response_builder = Response::builder().status(status);

    // Forward response headers (content-type, etc.)
    for (name, value) in upstream_response.headers() {
        let name_str = name.as_str().to_lowercase();
        if matches!(
            name_str.as_str(),
            "content-type" | "transfer-encoding" | "x-request-id"
        ) && let Ok(v) = value.to_str()
        {
            response_builder = response_builder.header(name.as_str(), v);
        }
    }

    // Stream the response body
    let body_stream = upstream_response.bytes_stream();
    let body = Body::from_stream(body_stream);

    response_builder
        .body(body)
        .map_err(|e| AppError::Internal(format!("Response build failed: {e}")))
        .map(IntoResponse::into_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: PLAN-028-1.4 — known provider types resolve correctly
    #[test]
    fn known_providers_resolve() {
        assert!(get_provider_mapping("anthropic").is_some());
        assert!(get_provider_mapping("openai").is_some());
        assert!(get_provider_mapping("google").is_some());
    }

    // @zen-test: PLAN-028-1.4 — unknown provider type rejected
    #[test]
    fn unknown_provider_rejected() {
        assert!(get_provider_mapping("unknown").is_none());
        assert!(get_provider_mapping("").is_none());
        assert!(get_provider_mapping("azure").is_none());
    }

    // @zen-test: PLAN-028-1.4 — anthropic uses x-api-key header
    #[test]
    fn anthropic_auth_header() {
        let m = get_provider_mapping("anthropic").unwrap();
        assert_eq!(m.auth_header_name, "x-api-key");
        assert_eq!(m.auth_header_prefix, "");
    }

    // @zen-test: PLAN-028-1.4 — openai uses Bearer auth
    #[test]
    fn openai_auth_header() {
        let m = get_provider_mapping("openai").unwrap();
        assert_eq!(m.auth_header_name, "authorization");
        assert_eq!(m.auth_header_prefix, "Bearer ");
    }

    // @zen-test: PLAN-028-1.4 — google uses x-goog-api-key header
    #[test]
    fn google_auth_header() {
        let m = get_provider_mapping("google").unwrap();
        assert_eq!(m.auth_header_name, "x-goog-api-key");
        assert_eq!(m.auth_header_prefix, "");
    }
}
