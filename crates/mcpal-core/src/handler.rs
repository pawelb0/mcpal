use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::{ErrorData, ListRootsResult, Root};
use rmcp::service::RequestContext;

/// Default client-side handler. Overrides only `list_roots` so MCP servers
/// that ask for workspace roots get the ones the user opted into via
/// `--root`. Everything else (elicitation, sampling, notifications) keeps
/// rmcp's safe defaults (decline / method-not-found / no-op).
#[derive(Clone, Default)]
pub struct Handler {
    roots: Vec<String>,
}

impl Handler {
    pub fn with_roots(mut self, roots: Vec<String>) -> Self {
        self.roots = roots;
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
}
