// @awa-component: PLAN-031-OAuthHandler
//
//! OAuth callback handler for Google OAuth flow.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::error::AppError;

/// Query parameters for OAuth callback.
#[derive(serde::Deserialize)]
pub struct OAuthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// `GET /auth/oauth/mcp/callback` — OAuth callback from Google.
// @awa-impl: PLAN-031 Phase 5.2
pub async fn oauth_callback_handler(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallbackParams>,
) -> Response {
    match handle_callback_inner(&state, params).await {
        Ok(server_id) => {
            // Return HTML that closes the window and signals success to the opener
            let html = format!(
                r#"<!DOCTYPE html>
<html><head><title>OAuth Complete</title></head>
<body>
<h2>Authorization successful!</h2>
<p>You can close this window.</p>
<script>
  if (window.opener) {{
    window.opener.postMessage({{ type: 'oauth-success', serverId: '{}' }}, '*');
    window.close();
  }} else {{
    document.querySelector('p').textContent = 'Authorization complete. You can close this tab and return to the app.';
  }}
</script>
</body></html>"#,
                server_id,
            );
            (
                StatusCode::OK,
                [("content-type", "text/html; charset=utf-8")],
                html,
            )
                .into_response()
        }
        Err(e) => {
            let html = format!(
                r#"<!DOCTYPE html>
<html><head><title>OAuth Error</title></head>
<body>
<h2>Authorization failed</h2>
<p>{}</p>
<script>
  if (window.opener) {{
    window.opener.postMessage({{ type: 'oauth-error', error: '{}' }}, '*');
    window.close();
  }} else {{
    document.querySelector('p').textContent += ' You can close this tab and return to the app.';
  }}
</script>
</body></html>"#,
                e, e,
            );
            (
                StatusCode::BAD_REQUEST,
                [("content-type", "text/html; charset=utf-8")],
                html,
            )
                .into_response()
        }
    }
}

/// Inner handler that returns `Result` for cleaner error handling.
async fn handle_callback_inner(
    state: &AppState,
    params: OAuthCallbackParams,
) -> Result<String, AppError> {
    // Check for OAuth error response
    if let Some(error) = params.error {
        return Err(AppError::Validation(format!(
            "OAuth provider returned error: {error}"
        )));
    }

    let code = params
        .code
        .ok_or_else(|| AppError::Validation("Missing authorization code".into()))?;
    let state_param = params
        .state
        .ok_or_else(|| AppError::Validation("Missing state parameter".into()))?;

    // Look up pending PKCE state
    let pending = state.oauth_state.take(&state_param).ok_or_else(|| {
        AppError::Validation("Invalid or expired OAuth state — please retry authorization".into())
    })?;

    // Parse OAuth config to get token_url and client_id
    let oauth_config: nize_core::models::mcp::OAuthConfig =
        serde_json::from_value(pending.oauth_config_json)
            .map_err(|e| AppError::Internal(format!("Invalid stored OAuth config: {e}")))?;

    // Exchange authorization code for tokens
    let token_resp = nize_core::mcp::oauth::exchange_authorization_code(
        &oauth_config.token_url,
        &oauth_config.client_id,
        &pending.client_secret,
        &code,
        &pending.redirect_uri,
        &pending.pkce_verifier,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Token exchange failed: {e}")))?;

    // Encrypt tokens
    let key = &state.config.mcp_encryption_key;

    let id_token_encrypted = match &token_resp.id_token {
        Some(t) => Some(
            nize_core::mcp::secrets::encrypt(t, key)
                .map_err(|e| AppError::Internal(format!("Encrypt id_token: {e}")))?,
        ),
        None => None,
    };

    let access_token_encrypted = nize_core::mcp::secrets::encrypt(&token_resp.access_token, key)
        .map_err(|e| AppError::Internal(format!("Encrypt access_token: {e}")))?;

    let refresh_token_encrypted = match &token_resp.refresh_token {
        Some(t) => Some(
            nize_core::mcp::secrets::encrypt(t, key)
                .map_err(|e| AppError::Internal(format!("Encrypt refresh_token: {e}")))?,
        ),
        None => None,
    };

    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(token_resp.expires_in);
    let scopes: Vec<String> = token_resp
        .scope
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(String::from)
        .collect();

    // Store tokens in database
    nize_core::mcp::queries::store_oauth_token(
        &state.pool,
        &pending.user_id,
        &pending.server_id,
        id_token_encrypted.as_deref(),
        &access_token_encrypted,
        refresh_token_encrypted.as_deref(),
        expires_at,
        &scopes,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Failed to store tokens: {e}")))?;

    tracing::info!(
        server_id = %pending.server_id,
        user_id = %pending.user_id,
        "OAuth tokens stored successfully"
    );

    // Mark server as available now that OAuth tokens are stored
    if let Err(e) = nize_core::mcp::queries::update_server(
        &state.pool,
        &pending.server_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true),
        None,
    )
    .await
    {
        tracing::warn!("Failed to mark server as available after OAuth: {e}");
    }

    // Discover and store tools now that we have valid OAuth tokens
    let oauth_headers =
        token_resp
            .id_token
            .as_ref()
            .map(|id_token| nize_core::mcp::execution::OAuthHeaders {
                id_token: id_token.clone(),
                access_token: token_resp.access_token.clone(),
            });
    discover_tools_after_oauth(state, &pending.server_id, oauth_headers.as_ref()).await;

    Ok(pending.server_id)
}

/// Discover and store tools from an OAuth-authenticated MCP server.
///
/// Called after a successful OAuth token exchange. Loads the server config,
/// runs `initialize` + `tools/list` with the bearer token, stores results,
/// and generates embeddings. Failures are logged but do not fail the callback.
async fn discover_tools_after_oauth(
    state: &AppState,
    server_id: &str,
    oauth_headers: Option<&nize_core::mcp::execution::OAuthHeaders>,
) {
    use crate::services::mcp_config;
    use nize_core::models::mcp::ServerConfig;

    // Load server to get its config (URL, etc.)
    let server = match nize_core::mcp::queries::get_server(&state.pool, server_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            tracing::warn!("OAuth tool discovery: server {server_id} not found");
            return;
        }
        Err(e) => {
            tracing::warn!("OAuth tool discovery: failed to load server {server_id}: {e}");
            return;
        }
    };

    let config_json = match server.config {
        Some(c) => c,
        None => {
            tracing::warn!("OAuth tool discovery: server {server_id} has no config");
            return;
        }
    };

    let config: ServerConfig = match serde_json::from_value(config_json) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("OAuth tool discovery: bad config for server {server_id}: {e}");
            return;
        }
    };

    let test_result = mcp_config::test_connection(&config, None, oauth_headers).await;

    if !test_result.success {
        tracing::warn!(
            "OAuth tool discovery: connection test failed for server {server_id}: {:?}",
            test_result.error,
        );
        return;
    }

    if !test_result.tools.is_empty() {
        let sid = server.id.to_string();
        if let Err(e) =
            mcp_config::store_tools_from_test(&state.pool, &sid, &test_result.tools).await
        {
            tracing::warn!("OAuth tool discovery: failed to store tools for {server_id}: {e}");
        }

        // Generate embeddings for the newly stored tools
        if let Err(e) = nize_core::embedding::indexer::embed_server_tools(
            &state.pool,
            &state.config_cache,
            &sid,
            &state.config.mcp_encryption_key,
        )
        .await
        {
            tracing::warn!("OAuth tool discovery: failed to embed tools for {server_id}: {e}");
        }

        tracing::info!(
            server_id = %server_id,
            tool_count = test_result.tools.len(),
            "OAuth tool discovery complete"
        );
    } else {
        tracing::info!(server_id = %server_id, "OAuth tool discovery: no tools returned");
    }
}
