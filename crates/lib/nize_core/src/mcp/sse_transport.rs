// @zen-component: XMCP-SseClientTransport
//
//! Legacy SSE client transport for MCP servers.
//!
//! Implements the legacy MCP SSE protocol:
//! 1. Client sends GET to SSE endpoint → receives SSE event stream
//! 2. Server sends an `endpoint` event containing the message URL
//! 3. Client POSTs JSON-RPC messages to that endpoint URL
//! 4. Server sends JSON-RPC responses & notifications via the SSE stream

use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;

use futures_util::StreamExt;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::service::RoleClient;
use rmcp::transport::Transport;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

/// Error type for SSE transport operations.
#[derive(Debug)]
pub enum SseTransportError {
    /// The SSE connection was never established or has been closed.
    NotConnected,
    /// The endpoint URL was not received from the server.
    NoEndpoint,
    /// Failed to send a message via HTTP POST.
    SendFailed(String),
    /// Failed to receive a message from the SSE stream.
    ReceiveFailed(String),
    /// The transport has been closed.
    Closed,
}

impl std::fmt::Display for SseTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConnected => write!(f, "SSE transport not connected"),
            Self::NoEndpoint => write!(f, "No endpoint URL received from SSE server"),
            Self::SendFailed(e) => write!(f, "SSE send failed: {e}"),
            Self::ReceiveFailed(e) => write!(f, "SSE receive failed: {e}"),
            Self::Closed => write!(f, "SSE transport closed"),
        }
    }
}

impl std::error::Error for SseTransportError {}

/// Legacy SSE client transport for MCP.
///
/// Connects to a legacy MCP SSE server endpoint, discovers the message
/// posting URL from the `endpoint` SSE event, and routes messages
/// between the rmcp service layer and the SSE server.
pub struct SseClientTransport {
    /// Base URL for the SSE endpoint (e.g. `http://host:port/sse`).
    base_url: String,
    /// HTTP client for GET (SSE stream) and POST (messages).
    client: reqwest::Client,
    /// Custom headers to include in requests.
    extra_headers: reqwest::header::HeaderMap,
    /// Receiver for incoming server messages (populated by background worker).
    rx: Arc<Mutex<mpsc::UnboundedReceiver<ServerJsonRpcMessage>>>,
    /// Sender for outgoing client messages (consumed by background worker).
    tx: Option<mpsc::UnboundedSender<ClientJsonRpcMessage>>,
    /// Cancellation token for the background worker.
    cancel: CancellationToken,
    /// Whether the transport has been started (first send triggers connection).
    started: bool,
    /// Oneshot for the background worker to signal it discovered the endpoint.
    endpoint_rx: Option<oneshot::Receiver<String>>,
}

impl SseClientTransport {
    /// Create a new SSE client transport.
    ///
    /// # Arguments
    /// * `base_url` — The SSE endpoint URL (e.g. `http://host:port/sse`)
    pub fn new(base_url: &str) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
            extra_headers: reqwest::header::HeaderMap::new(),
            rx: Arc::new(Mutex::new(rx)),
            tx: None,
            cancel: CancellationToken::new(),
            started: false,
            endpoint_rx: None,
        }
    }

    /// Create a new SSE client transport with a custom reqwest client and headers.
    pub fn with_client(
        client: reqwest::Client,
        base_url: &str,
        extra_headers: reqwest::header::HeaderMap,
    ) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self {
            base_url: base_url.to_string(),
            client,
            extra_headers,
            rx: Arc::new(Mutex::new(rx)),
            tx: None,
            cancel: CancellationToken::new(),
            started: false,
            endpoint_rx: None,
        }
    }

    /// Start the background worker that connects to the SSE endpoint.
    fn start(&mut self) {
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel::<ServerJsonRpcMessage>();
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel::<ClientJsonRpcMessage>();
        let (endpoint_tx, endpoint_rx) = oneshot::channel::<String>();

        self.rx = Arc::new(Mutex::new(incoming_rx));
        self.tx = Some(outgoing_tx);
        self.endpoint_rx = Some(endpoint_rx);

        let base_url = self.base_url.clone();
        let client = self.client.clone();
        let extra_headers = self.extra_headers.clone();
        let cancel = self.cancel.clone();

        tokio::spawn(async move {
            if let Err(e) = run_worker(
                base_url,
                client,
                extra_headers,
                incoming_tx,
                outgoing_rx,
                endpoint_tx,
                cancel.clone(),
            )
            .await
            {
                warn!("SSE transport worker exited with error: {e}");
            }
            cancel.cancel();
        });

        self.started = true;
    }
}

// @zen-impl: PLAN-033 Step 1.2 — Transport<RoleClient> for SseClientTransport
impl Transport<RoleClient> for SseClientTransport {
    type Error = SseTransportError;

    fn name() -> Cow<'static, str> {
        "SseClientTransport".into()
    }

    // @zen-impl: PLAN-033 T-XMCP-032 — send JSON-RPC messages via POST
    fn send(
        &mut self,
        item: rmcp::service::TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        // Start the background worker on first send
        if !self.started {
            self.start();
        }

        let tx = self.tx.clone();
        async move {
            let sender = tx.ok_or(SseTransportError::NotConnected)?;
            sender.send(item).map_err(|_| SseTransportError::Closed)
        }
    }

    // @zen-impl: PLAN-033 T-XMCP-033 — receive JSON-RPC messages from SSE stream
    fn receive(
        &mut self,
    ) -> impl Future<Output = Option<rmcp::service::RxJsonRpcMessage<RoleClient>>> + Send {
        // Start the background worker if not started
        if !self.started {
            self.start();
        }

        let rx = Arc::clone(&self.rx);
        let cancel = self.cancel.clone();
        async move {
            let mut guard = rx.lock().await;
            tokio::select! {
                msg = guard.recv() => msg,
                _ = cancel.cancelled() => None,
            }
        }
    }

    // @zen-impl: PLAN-033 T-XMCP-034 — close transport
    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.cancel.cancel();
        self.tx = None;
        async { Ok(()) }
    }
}

/// Background worker that manages the SSE connection.
///
/// 1. Connects to the SSE endpoint via GET
/// 2. Waits for the `endpoint` event to discover the message URL
/// 3. Reads incoming SSE events and forwards server messages
/// 4. Reads outgoing messages and POSTs them to the endpoint URL
// @zen-impl: PLAN-033 T-XMCP-031 — background worker
async fn run_worker(
    base_url: String,
    client: reqwest::Client,
    extra_headers: reqwest::header::HeaderMap,
    incoming_tx: mpsc::UnboundedSender<ServerJsonRpcMessage>,
    mut outgoing_rx: mpsc::UnboundedReceiver<ClientJsonRpcMessage>,
    endpoint_tx: oneshot::Sender<String>,
    cancel: CancellationToken,
) -> Result<(), SseTransportError> {
    // Connect to SSE endpoint
    let response = client
        .get(&base_url)
        .headers(extra_headers.clone())
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| SseTransportError::SendFailed(format!("SSE GET failed: {e}")))?;

    if !response.status().is_success() {
        return Err(SseTransportError::SendFailed(format!(
            "SSE GET returned status {}",
            response.status()
        )));
    }

    // Parse response body as SSE stream
    let byte_stream = response.bytes_stream();
    let mut sse_stream = sse_stream::SseStream::from_byte_stream(byte_stream);

    // Wait for the `endpoint` event
    let endpoint_url = discover_endpoint(&base_url, &mut sse_stream, &cancel).await?;
    debug!(endpoint = %endpoint_url, "SSE endpoint discovered");
    let _ = endpoint_tx.send(endpoint_url.clone());

    // Wrap the endpoint URL for shared access across tasks
    let endpoint_url = Arc::new(endpoint_url);

    // Spawn a task to POST outgoing messages
    let post_client = client.clone();
    let post_headers = extra_headers.clone();
    let post_endpoint = Arc::clone(&endpoint_url);
    let post_cancel = cancel.clone();
    let post_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = outgoing_rx.recv() => {
                    match msg {
                        Some(message) => {
                            if let Err(e) = post_message(
                                &post_client,
                                &post_endpoint,
                                &post_headers,
                                &message,
                            ).await {
                                warn!("SSE POST failed: {e}");
                                break;
                            }
                        }
                        None => break, // Channel closed
                    }
                }
                _ = post_cancel.cancelled() => break,
            }
        }
    });

    // Read incoming SSE events and forward to the service layer
    loop {
        tokio::select! {
            event = sse_stream.next() => {
                match event {
                    Some(Ok(sse)) => {
                        if let Some(msg) = parse_sse_message(&sse) {
                            if incoming_tx.send(msg).is_err() {
                                break; // Receiver dropped
                            }
                        }
                        // Ignore non-message events (endpoint already handled)
                    }
                    Some(Err(e)) => {
                        warn!("SSE stream error: {e}");
                        break;
                    }
                    None => {
                        debug!("SSE stream ended");
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }

    cancel.cancel();
    post_handle.abort();
    Ok(())
}

/// Discover the endpoint URL from the SSE stream.
///
/// Reads events until it finds one with `event: endpoint` and extracts the URL
/// from the `data` field.
async fn discover_endpoint<S>(
    base_url: &str,
    sse_stream: &mut S,
    cancel: &CancellationToken,
) -> Result<String, SseTransportError>
where
    S: futures_util::Stream<Item = Result<sse_stream::Sse, sse_stream::Error>> + Unpin,
{
    loop {
        tokio::select! {
            event = sse_stream.next() => {
                match event {
                    Some(Ok(sse)) => {
                        if sse.event.as_deref() == Some("endpoint") {
                            if let Some(data) = &sse.data {
                                let endpoint = resolve_endpoint_url(base_url, data);
                                return Ok(endpoint);
                            }
                            return Err(SseTransportError::NoEndpoint);
                        }
                        // Skip non-endpoint events during discovery
                    }
                    Some(Err(e)) => {
                        return Err(SseTransportError::ReceiveFailed(
                            format!("SSE stream error during endpoint discovery: {e}")
                        ));
                    }
                    None => {
                        return Err(SseTransportError::ReceiveFailed(
                            "SSE stream ended before endpoint event".into()
                        ));
                    }
                }
            }
            _ = cancel.cancelled() => {
                return Err(SseTransportError::Closed);
            }
        }
    }
}

/// Resolve the endpoint URL relative to the base URL.
///
/// If the endpoint data is an absolute URL, return it as-is.
/// If it's a relative path, resolve it against the base URL's origin.
fn resolve_endpoint_url(base_url: &str, endpoint_data: &str) -> String {
    let trimmed = endpoint_data.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    // Resolve relative URL against base
    if let Ok(base) = url::Url::parse(base_url) {
        if let Ok(resolved) = base.join(trimmed) {
            return resolved.to_string();
        }
    }
    // Fallback: just return as-is
    trimmed.to_string()
}

/// Parse an SSE event into a ServerJsonRpcMessage.
///
/// Only `message` events (event type is None or "message") with JSON data are parsed.
fn parse_sse_message(sse: &sse_stream::Sse) -> Option<ServerJsonRpcMessage> {
    // Only parse events that are message-type (no event field, or event == "message")
    match sse.event.as_deref() {
        None | Some("message") => {}
        _ => return None, // Skip endpoint, ping, and other event types
    }

    let data = sse.data.as_deref()?;
    match serde_json::from_str::<ServerJsonRpcMessage>(data) {
        Ok(msg) => Some(msg),
        Err(e) => {
            warn!("Failed to parse SSE message as JSON-RPC: {e}");
            None
        }
    }
}

/// POST a JSON-RPC message to the SSE message endpoint.
async fn post_message(
    client: &reqwest::Client,
    endpoint_url: &str,
    extra_headers: &reqwest::header::HeaderMap,
    message: &ClientJsonRpcMessage,
) -> Result<(), SseTransportError> {
    let response = client
        .post(endpoint_url)
        .headers(extra_headers.clone())
        .json(message)
        .send()
        .await
        .map_err(|e| {
            SseTransportError::SendFailed(format!("POST to {endpoint_url} failed: {e}"))
        })?;

    if !response.status().is_success() {
        return Err(SseTransportError::SendFailed(format!(
            "POST to {} returned status {}",
            endpoint_url,
            response.status()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: PLAN-033 T-XMCP-038 — endpoint discovery from SSE endpoint event
    #[test]
    fn resolve_endpoint_url_absolute() {
        let result = resolve_endpoint_url(
            "http://localhost:3000/sse",
            "http://localhost:3000/message?sessionId=abc",
        );
        assert_eq!(result, "http://localhost:3000/message?sessionId=abc");
    }

    // @zen-test: PLAN-033 T-XMCP-038 — endpoint discovery relative URL
    #[test]
    fn resolve_endpoint_url_relative() {
        let result = resolve_endpoint_url("http://localhost:3000/sse", "/message?sessionId=abc");
        assert_eq!(result, "http://localhost:3000/message?sessionId=abc");
    }

    // @zen-test: PLAN-033 T-XMCP-038 — endpoint discovery relative path
    #[test]
    fn resolve_endpoint_url_relative_no_slash() {
        let result = resolve_endpoint_url("http://localhost:3000/sse", "message?sessionId=abc");
        assert_eq!(result, "http://localhost:3000/message?sessionId=abc");
    }

    // @zen-test: PLAN-033 T-XMCP-038 — endpoint discovery trims whitespace
    #[test]
    fn resolve_endpoint_url_trims_whitespace() {
        let result =
            resolve_endpoint_url("http://localhost:3000/sse", "  /message?sessionId=abc  ");
        assert_eq!(result, "http://localhost:3000/message?sessionId=abc");
    }

    // @zen-test: PLAN-033 T-XMCP-037 — SSE event parsing: message event with JSON-RPC
    #[test]
    fn parse_sse_message_valid_jsonrpc() {
        let sse = sse_stream::Sse {
            event: Some("message".to_string()),
            data: Some(r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"test","version":"1.0"}}}"#.to_string()),
            id: None,
            retry: None,
        };
        let msg = parse_sse_message(&sse);
        assert!(msg.is_some());
    }

    // @zen-test: PLAN-033 T-XMCP-037 — SSE event parsing: default event (no event field)
    #[test]
    fn parse_sse_message_no_event_field() {
        let sse = sse_stream::Sse {
            event: None,
            data: Some(
                r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#.to_string(),
            ),
            id: None,
            retry: None,
        };
        let msg = parse_sse_message(&sse);
        assert!(msg.is_some());
    }

    // @zen-test: PLAN-033 T-XMCP-037 — SSE event parsing: skip endpoint events
    #[test]
    fn parse_sse_message_skips_endpoint_event() {
        let sse = sse_stream::Sse {
            event: Some("endpoint".to_string()),
            data: Some("/message?sessionId=abc".to_string()),
            id: None,
            retry: None,
        };
        let msg = parse_sse_message(&sse);
        assert!(msg.is_none());
    }

    // @zen-test: PLAN-033 T-XMCP-037 — SSE event parsing: skip events without data
    #[test]
    fn parse_sse_message_no_data() {
        let sse = sse_stream::Sse {
            event: Some("message".to_string()),
            data: None,
            id: None,
            retry: None,
        };
        let msg = parse_sse_message(&sse);
        assert!(msg.is_none());
    }

    // @zen-test: PLAN-033 T-XMCP-037 — SSE event parsing: invalid JSON data
    #[test]
    fn parse_sse_message_invalid_json() {
        let sse = sse_stream::Sse {
            event: Some("message".to_string()),
            data: Some("not json".to_string()),
            id: None,
            retry: None,
        };
        let msg = parse_sse_message(&sse);
        assert!(msg.is_none());
    }

    // @zen-test: PLAN-033 T-XMCP-039 — error: transport not started
    #[test]
    fn sse_transport_error_display() {
        let e = SseTransportError::NotConnected;
        assert_eq!(e.to_string(), "SSE transport not connected");

        let e = SseTransportError::NoEndpoint;
        assert_eq!(e.to_string(), "No endpoint URL received from SSE server");

        let e = SseTransportError::Closed;
        assert_eq!(e.to_string(), "SSE transport closed");
    }

    // @zen-test: PLAN-033 T-XMCP-039 — new transport is not started
    #[test]
    fn sse_transport_new_state() {
        let transport = SseClientTransport::new("http://localhost:3000/sse");
        assert!(!transport.started);
        assert!(transport.tx.is_none());
    }

    // @zen-test: PLAN-033 T-XMCP-038 — endpoint discovery from mock SSE stream
    #[tokio::test]
    async fn discover_endpoint_from_stream() {
        let events = vec![Ok(sse_stream::Sse {
            event: Some("endpoint".to_string()),
            data: Some("/message?sessionId=test123".to_string()),
            id: None,
            retry: None,
        })];
        let mut stream = futures_util::stream::iter(events);
        let cancel = CancellationToken::new();
        let result = discover_endpoint("http://localhost:3000/sse", &mut stream, &cancel).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "http://localhost:3000/message?sessionId=test123"
        );
    }

    // @zen-test: PLAN-033 T-XMCP-039 — endpoint discovery fails on empty stream
    #[tokio::test]
    async fn discover_endpoint_empty_stream() {
        let events: Vec<Result<sse_stream::Sse, sse_stream::Error>> = vec![];
        let mut stream = futures_util::stream::iter(events);
        let cancel = CancellationToken::new();
        let result = discover_endpoint("http://localhost:3000/sse", &mut stream, &cancel).await;
        assert!(result.is_err());
    }

    // @zen-test: PLAN-033 T-XMCP-039 — endpoint discovery skips non-endpoint events
    #[tokio::test]
    async fn discover_endpoint_skips_message_events() {
        let events = vec![
            Ok(sse_stream::Sse {
                event: Some("message".to_string()),
                data: Some(r#"{"jsonrpc":"2.0"}"#.to_string()),
                id: None,
                retry: None,
            }),
            Ok(sse_stream::Sse {
                event: Some("endpoint".to_string()),
                data: Some("/msg?sid=x".to_string()),
                id: None,
                retry: None,
            }),
        ];
        let mut stream = futures_util::stream::iter(events);
        let cancel = CancellationToken::new();
        let result = discover_endpoint("http://localhost:8080/sse", &mut stream, &cancel).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://localhost:8080/msg?sid=x");
    }

    // @zen-test: PLAN-033 T-XMCP-039 — endpoint discovery cancelled
    #[tokio::test]
    async fn discover_endpoint_cancelled() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        // Use a stream that would hang forever
        let mut stream =
            futures_util::stream::pending::<Result<sse_stream::Sse, sse_stream::Error>>();
        let result = discover_endpoint("http://localhost:3000/sse", &mut stream, &cancel).await;
        assert!(matches!(result, Err(SseTransportError::Closed)));
    }
}
