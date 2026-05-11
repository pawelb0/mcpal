use std::io::{BufRead, IsTerminal};

use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::{
    CreateElicitationRequestParams, CreateElicitationResult, CreateMessageRequestParams,
    CreateMessageResult, ElicitationAction, ErrorData, ListRootsResult,
    LoggingMessageNotificationParam, Root,
};
use rmcp::service::{NotificationContext, RequestContext};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Client-side hooks for server-initiated traffic.
///
/// Overrides:
///   - `list_roots` returns user-configured roots
///   - `create_elicitation` prompts the user on a TTY, or declines when
///     interactivity is off / there is no terminal
///   - `create_message` (sampling) delegates to an external program when
///     `sampling_handler` is set; otherwise it returns method-not-found
///   - `on_logging_message` routes server logs to stderr
#[derive(Clone, Default)]
pub struct Handler {
    roots: Vec<String>,
    interactive: bool,
    sampling_handler: Option<Vec<String>>,
}

impl Handler {
    pub fn with_roots(mut self, roots: Vec<String>) -> Self {
        self.roots = roots;
        self
    }

    pub fn interactive(mut self, enabled: bool) -> Self {
        self.interactive = enabled;
        self
    }

    /// Set the external program that handles `sampling/createMessage`. The
    /// first element is the executable, the rest are positional args. mcpal
    /// pipes the request params as JSON on stdin and reads
    /// `CreateMessageResult` JSON from stdout.
    pub fn sampling_handler(mut self, argv: Option<Vec<String>>) -> Self {
        self.sampling_handler = argv.filter(|v| !v.is_empty());
        self
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
        match request {
            CreateElicitationRequestParams::FormElicitationParams { message, .. } => {
                if !self.interactive || !std::io::stdin().is_terminal() {
                    return Ok(CreateElicitationResult::new(ElicitationAction::Decline));
                }
                eprintln!("[server elicitation] {message}");
                eprint!("> ");
                let line = tokio::task::spawn_blocking(|| {
                    let mut buf = String::new();
                    std::io::stdin().lock().read_line(&mut buf).map(|_| buf)
                })
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                match line {
                    Ok(buf) if !buf.is_empty() => {
                        Ok(CreateElicitationResult::new(ElicitationAction::Accept)
                            .with_content(json!({ "value": buf.trim() })))
                    }
                    _ => Ok(CreateElicitationResult::new(ElicitationAction::Cancel)),
                }
            }
            CreateElicitationRequestParams::UrlElicitationParams { url, message, .. } => {
                eprintln!("[server elicitation] {message}");
                eprintln!("  open: {url}");
                Ok(CreateElicitationResult::new(ElicitationAction::Accept))
            }
        }
    }

    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        _ctx: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, ErrorData> {
        let Some(argv) = self.sampling_handler.as_ref() else {
            return Err(ErrorData::method_not_found::<
                rmcp::model::CreateMessageRequestMethod,
            >());
        };
        run_sampling_handler(argv, &params)
            .await
            .map_err(|e| ErrorData::internal_error(format!("sampling handler: {e}"), None))
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _ctx: NotificationContext<RoleClient>,
    ) {
        let logger = params.logger.as_deref().unwrap_or("server");
        let data = serde_json::to_string(&params.data).unwrap_or_default();
        eprintln!("[{logger} {:?}] {data}", params.level);
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

    let stdin_payload = serde_json::to_vec(params).map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&stdin_payload)
            .await
            .map_err(|e| e.to_string())?;
        stdin.shutdown().await.map_err(|e| e.to_string())?;
    }

    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("handler exited {:?}", output.status.code()));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
