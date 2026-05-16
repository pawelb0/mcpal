use std::fs;
use std::io::Read;

use anyhow::{Context, Result};
use mcpal_core::rmcp::model::{ClientRequest, CustomRequest};
use serde_json::Value;

use crate::runtime::Ctx;

/// Send an arbitrary JSON-RPC request to the server and emit the result.
/// `params` is one of: inline JSON (`'{"k":"v"}'`), `@path/to/file.json`, or
/// `-` to read from stdin. Pure `curl`-for-MCP escape hatch.
pub async fn run(reference: &str, method: &str, params: Option<&str>, ctx: &Ctx) -> Result<()> {
    let params = parse_params(params)?;
    let (_, client) = ctx.open(reference).await?;
    let result = ctx
        .under_deadline(
            client.send_request(ClientRequest::CustomRequest(CustomRequest::new(
                method.to_string(),
                params,
            ))),
        )
        .await?
        .with_context(|| format!("raw {method}"))?;
    let v = serde_json::to_value(&result)?;
    ctx.render_one(&v)?;
    Ok(())
}

fn parse_params(spec: Option<&str>) -> Result<Option<Value>> {
    let Some(spec) = spec else {
        return Ok(None);
    };
    let text = if spec == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("read stdin")?;
        buf
    } else if let Some(path) = spec.strip_prefix('@') {
        fs::read_to_string(path).with_context(|| format!("read {path}"))?
    } else {
        spec.to_string()
    };
    let v: Value = serde_json::from_str(&text).context("parse params as JSON")?;
    Ok(Some(v))
}
