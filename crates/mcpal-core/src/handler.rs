use std::io::{BufRead, IsTerminal};

use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::{
    CreateElicitationRequestParams, CreateElicitationResult, ElicitationAction, ErrorData,
    ListRootsResult, LoggingMessageNotificationParam, Root,
};
use rmcp::service::{NotificationContext, RequestContext};
use serde_json::json;

/// Client-side hooks for server-initiated traffic.
///
/// Overrides:
///   - `list_roots` returns user-configured roots
///   - `create_elicitation` prompts the user on a TTY, or declines when
///     interactivity is off / there is no terminal
///   - `on_logging_message` routes server logs to stderr
///
/// `create_message` (sampling) keeps rmcp's `method_not_found` default until
/// the `--sampling-handler <cmd>` plugin lands.
#[derive(Clone, Default)]
pub struct Handler {
    roots: Vec<String>,
    interactive: bool,
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

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _ctx: NotificationContext<RoleClient>,
    ) {
        let logger = params.logger.as_deref().unwrap_or("server");
        let data = serde_json::to_string(&params.data).unwrap_or_else(|_| String::new());
        eprintln!("[{logger} {:?}] {data}", params.level);
    }
}
