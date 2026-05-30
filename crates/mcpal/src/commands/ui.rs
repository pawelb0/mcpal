//! Classify and surface UI/app payloads embedded in tool results.
//!
//! Covers two coexisting ecosystems:
//!   - mcp-ui: a content block whose embedded resource carries a `ui://`
//!     URI and HTML (or JSON descriptor).
//!   - OpenAI Apps SDK: a content block whose embedded resource has a
//!     `application/vnd.openai.app+json` MIME type.

use std::io::Write;

use anyhow::{Context, Result};
use base64::Engine;
use mcpal_core::rmcp::model::{
    CallToolRequestParams, CallToolResult, RawContent, ResourceContents,
};
use serde::Serialize;
use serde_json::Value;

use crate::cli::UiAction;
use crate::runtime::Ctx;

#[derive(Serialize)]
pub struct Hit {
    pub index: usize,
    pub kind: HitKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub size_bytes: usize,
    #[serde(skip)]
    body: Body,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HitKind {
    McpUi,
    OpenAiApp,
    Resource,
    Image,
    Audio,
    Text,
    ResourceLink,
}

enum Body {
    Text(String),
    Blob(Vec<u8>),
    None,
}

/// Walk a tool result, classify each content block. Cheap; no I/O.
pub fn classify(result: &CallToolResult) -> Vec<Hit> {
    result
        .content
        .iter()
        .enumerate()
        .map(|(i, item)| classify_one(i, &item.raw))
        .collect()
}

fn classify_one(index: usize, raw: &RawContent) -> Hit {
    match raw {
        RawContent::Resource(emb) => {
            let (uri, mime, body, size) = match &emb.resource {
                ResourceContents::TextResourceContents {
                    uri,
                    mime_type,
                    text,
                    ..
                } => (
                    uri.clone(),
                    mime_type.clone(),
                    Body::Text(text.clone()),
                    text.len(),
                ),
                ResourceContents::BlobResourceContents {
                    uri,
                    mime_type,
                    blob,
                    ..
                } => {
                    let decoded = base64::engine::general_purpose::STANDARD
                        .decode(blob)
                        .unwrap_or_default();
                    let size = decoded.len();
                    (uri.clone(), mime_type.clone(), Body::Blob(decoded), size)
                }
            };
            let kind = if uri.starts_with("ui://") {
                HitKind::McpUi
            } else if mime
                .as_deref()
                .map(|m| m.contains("openai.app") || m.contains("openai+json"))
                .unwrap_or(false)
            {
                HitKind::OpenAiApp
            } else {
                HitKind::Resource
            };
            Hit {
                index,
                kind,
                uri: Some(uri),
                mime_type: mime,
                size_bytes: size,
                body,
            }
        }
        RawContent::Text(t) => Hit {
            index,
            kind: HitKind::Text,
            uri: None,
            mime_type: None,
            size_bytes: t.text.len(),
            body: Body::Text(t.text.clone()),
        },
        RawContent::Image(img) => Hit {
            index,
            kind: HitKind::Image,
            uri: None,
            mime_type: Some(img.mime_type.clone()),
            size_bytes: img.data.len(),
            body: Body::None,
        },
        RawContent::Audio(a) => Hit {
            index,
            kind: HitKind::Audio,
            uri: None,
            mime_type: Some(a.mime_type.clone()),
            size_bytes: a.data.len(),
            body: Body::None,
        },
        RawContent::ResourceLink(r) => Hit {
            index,
            kind: HitKind::ResourceLink,
            uri: Some(r.uri.clone()),
            mime_type: None,
            size_bytes: 0,
            body: Body::None,
        },
    }
}

/// True when a result is worth flagging — at least one mcp-ui or
/// OpenAI-Apps content block. Used by the TUI to badge call results.
pub fn has_ui(result: &CallToolResult) -> bool {
    classify(result)
        .iter()
        .any(|h| matches!(h.kind, HitKind::McpUi | HitKind::OpenAiApp))
}

pub async fn run(action: UiAction, ctx: &Ctx) -> Result<()> {
    match action {
        UiAction::Inspect {
            reference,
            name,
            params,
            save,
            open,
        } => inspect(&reference, &name, params.as_deref(), save, open, ctx).await,
    }
}

async fn inspect(
    reference: &str,
    tool: &str,
    params: Option<&str>,
    save: bool,
    open: bool,
    ctx: &Ctx,
) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let mut req = CallToolRequestParams::new(tool.to_string());
    if let Some(p) = params {
        let v: Value = serde_json::from_str(p).context("--params is not valid JSON")?;
        if let Value::Object(m) = v {
            req = req.with_arguments(m);
        }
    }
    let result = ctx.under_deadline(client.call_tool(req)).await??;
    let hits = classify(&result);

    #[derive(Serialize)]
    struct Summary<'a> {
        reference: &'a str,
        tool: &'a str,
        ui_resources: usize,
        is_error: bool,
        hits: &'a [Hit],
    }

    ctx.render_one(&Summary {
        reference,
        tool,
        ui_resources: hits
            .iter()
            .filter(|h| matches!(h.kind, HitKind::McpUi | HitKind::OpenAiApp))
            .count(),
        is_error: result.is_error.unwrap_or(false),
        hits: &hits,
    })?;

    if save || open {
        save_artifacts(&hits, open)?;
    }
    Ok(())
}

fn save_artifacts(hits: &[Hit], also_open: bool) -> Result<()> {
    for h in hits {
        let bytes: &[u8] = match &h.body {
            Body::Text(t) => t.as_bytes(),
            Body::Blob(b) => b.as_slice(),
            Body::None => continue,
        };
        if !matches!(h.kind, HitKind::McpUi | HitKind::OpenAiApp) {
            continue;
        }
        let ext = match h.mime_type.as_deref() {
            Some(m) if m.contains("json") => "json",
            Some(m) if m.contains("javascript") => "js",
            _ => "html",
        };
        let path = format!("/tmp/mcpal-ui-{}-{}.{}", std::process::id(), h.index, ext);
        let mut f = std::fs::File::create(&path).with_context(|| format!("create {path}"))?;
        f.write_all(bytes)?;
        eprintln!("saved hit #{}: {path}", h.index);
        if also_open {
            let _ = std::process::Command::new(opener()).arg(&path).spawn();
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn opener() -> &'static str {
    "open"
}
#[cfg(target_os = "linux")]
fn opener() -> &'static str {
    "xdg-open"
}
#[cfg(target_os = "windows")]
fn opener() -> &'static str {
    "explorer"
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(v: serde_json::Value) -> CallToolResult {
        serde_json::from_value(v).expect("valid CallToolResult JSON")
    }

    #[test]
    fn text_block_classified_as_text() {
        let r = parse(json!({
            "content": [{ "type": "text", "text": "hello" }]
        }));
        let hits = classify(&r);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, HitKind::Text);
        assert_eq!(hits[0].size_bytes, 5);
    }

    #[test]
    fn ui_uri_classified_as_mcp_ui() {
        let r = parse(json!({
            "content": [{
                "type": "resource",
                "resource": {
                    "uri": "ui://weather/london",
                    "mimeType": "text/html",
                    "text": "<html></html>"
                }
            }]
        }));
        let hits = classify(&r);
        assert_eq!(hits[0].kind, HitKind::McpUi);
        assert_eq!(hits[0].uri.as_deref(), Some("ui://weather/london"));
        assert!(has_ui(&r));
    }

    #[test]
    fn vnd_openai_mime_classified_as_openai_app() {
        let r = parse(json!({
            "content": [{
                "type": "resource",
                "resource": {
                    "uri": "openai://app/x",
                    "mimeType": "application/vnd.openai.app+json",
                    "text": "{\"component\":\"X\"}"
                }
            }]
        }));
        let hits = classify(&r);
        assert_eq!(hits[0].kind, HitKind::OpenAiApp);
        assert!(has_ui(&r));
    }

    #[test]
    fn plain_resource_not_flagged() {
        let r = parse(json!({
            "content": [{
                "type": "resource",
                "resource": {
                    "uri": "file:///etc/hosts",
                    "mimeType": "text/plain",
                    "text": "127.0.0.1 localhost"
                }
            }]
        }));
        let hits = classify(&r);
        assert_eq!(hits[0].kind, HitKind::Resource);
        assert!(!has_ui(&r));
    }

    #[test]
    fn mixed_payload_counts_ui_correctly() {
        let r = parse(json!({
            "content": [
                { "type": "text", "text": "describe me" },
                { "type": "resource", "resource": {
                    "uri": "ui://x", "mimeType": "text/html", "text": "<p/>" }
                },
                { "type": "resource", "resource": {
                    "uri": "data://other", "mimeType": "application/octet-stream",
                    "blob": "" }
                }
            ]
        }));
        let hits = classify(&r);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].kind, HitKind::Text);
        assert_eq!(hits[1].kind, HitKind::McpUi);
        assert_eq!(hits[2].kind, HitKind::Resource);
        assert!(has_ui(&r));
    }

}
