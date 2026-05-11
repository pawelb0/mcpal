use rmcp::{RoleClient, ServiceExt, service::RunningService, transport::TokioChildProcess};
use tokio::process::Command;

use crate::{Error, Result, ServerSpec};

pub type Client = RunningService<RoleClient, ()>;

pub async fn connect(spec: &ServerSpec) -> Result<Client> {
    match spec {
        ServerSpec::Stdio { command, args, env } => {
            let mut cmd = Command::new(command);
            cmd.args(args);
            for (k, v) in env {
                cmd.env(k, v);
            }
            let transport = TokioChildProcess::new(cmd)?;
            ().serve(transport)
                .await
                .map_err(|e| Error::Service(e.to_string()))
        }
        ServerSpec::Http { .. } => Err(Error::Unsupported("HTTP transport")),
    }
}
