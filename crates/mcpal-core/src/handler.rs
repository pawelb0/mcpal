use std::io::{BufRead, IsTerminal, Write};

use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::{
    CancelledNotificationParam, CreateElicitationRequestParams, CreateElicitationResult,
    CreateMessageRequestParams, CreateMessageResult, ElicitationAction, ErrorData, ListRootsResult,
    LoggingLevel, LoggingMessageNotificationParam, ProgressNotificationParam,
    ResourceUpdatedNotificationParam, Root,
};
use rmcp::service::{NotificationContext, RequestContext};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

/// Client-side hooks for server-initiated traffic. Override `list_roots`,
/// elicitation/sampling, and forward notifications via `events`.
#[derive(Clone, Default)]
pub struct Handler {
    pub roots: Vec<String>,
    pub interactive: bool,
    pub sampling_handler: Option<Vec<String>>,
    /// If set, the handler forwards every observed notification as a JSON
    /// `{kind, …}` document on this channel.
    pub events: Option<UnboundedSender<Value>>,
}

impl Handler {
    fn emit(&self, value: Value) {
        if let Some(tx) = &self.events {
            let _ = tx.send(value);
        }
    }

    /// Serialize the param struct and tag it with a `kind` field.
    fn tag(&self, kind: &str, params: impl serde::Serialize) {
        if self.events.is_none() {
            return;
        }
        let mut v = serde_json::to_value(&params).unwrap_or_else(|_| json!({}));
        if let Value::Object(ref mut m) = v {
            m.insert("kind".into(), Value::String(kind.into()));
            self.emit(v);
        }
    }
}

impl ClientHandler for Handler {
    async fn list_roots(
        &self,
        _ctx: RequestContext<RoleClient>,
    ) -> Result<ListRootsResult, ErrorData> {
        let roots: Vec<Root> = self.roots.iter().cloned().map(Root::new).collect();
        Ok(ListRootsResult::new(roots))
    }

    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        _ctx: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        self.tag("elicitation_request", &request);
        use CreateElicitationRequestParams::*;
        let result = match request {
            FormElicitationParams { message, .. } => {
                if !self.interactive || !std::io::stdin().is_terminal() {
                    CreateElicitationResult::new(ElicitationAction::Decline)
                } else {
                    eprintln!("[server elicitation] {message}");
                    eprint!("> ");
                    std::io::stderr().flush().ok();
                    let buf = tokio::task::spawn_blocking(|| {
                        let mut b = String::new();
                        std::io::stdin().lock().read_line(&mut b).map(|_| b)
                    })
                    .await
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
                    .unwrap_or_default();
                    if buf.is_empty() {
                        CreateElicitationResult::new(ElicitationAction::Cancel)
                    } else {
                        CreateElicitationResult::new(ElicitationAction::Accept)
                            .with_content(json!({"value": buf.trim()}))
                    }
                }
            }
            UrlElicitationParams { url, message, .. } => {
                eprintln!("[server elicitation] {message}\n  open: {url}");
                CreateElicitationResult::new(ElicitationAction::Accept)
            }
        };
        self.tag("elicitation_response", &result);
        Ok(result)
    }

    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        _ctx: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, ErrorData> {
        self.tag("sampling_request", &params);
        let argv = self.sampling_handler.as_ref().ok_or_else(|| {
            self.emit(json!({
                "kind": "sampling_response",
                "error": "no sampling handler configured",
            }));
            ErrorData::method_not_found::<rmcp::model::CreateMessageRequestMethod>()
        })?;
        match run_sampling_handler(argv, &params).await {
            Ok(result) => {
                self.tag("sampling_response", &result);
                Ok(result)
            }
            Err(e) => {
                let msg = format!("sampling handler: {e}");
                self.emit(json!({"kind": "sampling_response", "error": e}));
                Err(ErrorData::internal_error(msg, None))
            }
        }
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _ctx: NotificationContext<RoleClient>,
    ) {
        let logger = params.logger.as_deref().unwrap_or("server").to_string();
        let data = serde_json::to_string(&params.data).unwrap_or_default();
        use LoggingLevel::*;
        match params.level {
            Debug => tracing::debug!(target: "mcpal::server", logger, %data),
            Info | Notice => tracing::info!(target: "mcpal::server", logger, %data),
            Warning => tracing::warn!(target: "mcpal::server", logger, %data),
            Error | Critical | Alert | Emergency => {
                tracing::error!(target: "mcpal::server", logger, %data);
            }
        }
        self.tag("log", params);
    }

    async fn on_progress(&self, p: ProgressNotificationParam, _: NotificationContext<RoleClient>) {
        self.tag("progress", p);
    }
    async fn on_resource_updated(
        &self,
        p: ResourceUpdatedNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        self.tag("resource_updated", p);
    }
    async fn on_resource_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.emit(json!({"kind": "resource_list_changed"}));
    }
    async fn on_tool_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.emit(json!({"kind": "tool_list_changed"}));
    }
    async fn on_prompt_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.emit(json!({"kind": "prompt_list_changed"}));
    }
    async fn on_cancelled(
        &self,
        p: CancelledNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        self.tag("cancelled", p);
    }
}

async fn run_sampling_handler(
    argv: &[String],
    params: &CreateMessageRequestParams,
) -> Result<CreateMessageResult, String> {
    let (cmd, rest) = argv.split_first().ok_or("empty argv")?;
    let mut child = Command::new(cmd)
        .args(rest)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn {cmd}: {e}"))?;
    let payload = serde_json::to_vec(params).map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&payload).await.map_err(|e| e.to_string())?;
        stdin.shutdown().await.map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("handler exited {:?}", out.status.code()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}
