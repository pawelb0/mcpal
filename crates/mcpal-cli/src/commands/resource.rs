use anyhow::Result;
use mcpal_core::rmcp::model::{
    ReadResourceRequestParams, SubscribeRequestParams, UnsubscribeRequestParams,
};
use mcpal_output::{emit_list, emit_one};
use serde::Serialize;
use serde_json::json;

use crate::cli::{ResourceAction, ResourceTemplateAction};
use crate::runtime::Ctx;

pub async fn run(action: ResourceAction, ctx: &Ctx) -> Result<()> {
    match action {
        ResourceAction::List { reference } => list(&reference, ctx).await,
        ResourceAction::Read { reference, uri } => read(&reference, &uri, ctx).await,
        ResourceAction::Subscribe { reference, uri } => subscribe(&reference, &uri, ctx).await,
        ResourceAction::Unsubscribe { reference, uri } => unsubscribe(&reference, &uri, ctx).await,
        ResourceAction::Template { action } => match action {
            ResourceTemplateAction::List { reference } => templates(&reference, ctx).await,
        },
    }
}

#[derive(Serialize)]
struct ResourceSummary<'a> {
    uri: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime: Option<&'a str>,
}

#[derive(Serialize)]
struct TemplateSummary<'a> {
    uri_template: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime: Option<&'a str>,
}

async fn list(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let resources = client.list_all_resources().await?;
    let summaries: Vec<ResourceSummary<'_>> = resources
        .iter()
        .map(|r| ResourceSummary {
            uri: &r.uri,
            name: &r.name,
            mime: r.mime_type.as_deref(),
        })
        .collect();
    emit_list(ctx.format, &summaries, &[], |_| Vec::new())?;
    Ok(())
}

async fn read(reference: &str, uri: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let result = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await?;
    emit_one(ctx.format, &result)?;
    Ok(())
}

async fn subscribe(reference: &str, uri: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    client.subscribe(SubscribeRequestParams::new(uri)).await?;
    emit_one(ctx.format, &json!({"ok": true, "subscribed": uri}))?;
    Ok(())
}

async fn unsubscribe(reference: &str, uri: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    client
        .unsubscribe(UnsubscribeRequestParams::new(uri))
        .await?;
    emit_one(ctx.format, &json!({"ok": true, "unsubscribed": uri}))?;
    Ok(())
}

async fn templates(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let templates = client.list_all_resource_templates().await?;
    let summaries: Vec<TemplateSummary<'_>> = templates
        .iter()
        .map(|t| TemplateSummary {
            uri_template: &t.uri_template,
            name: &t.name,
            mime: t.mime_type.as_deref(),
        })
        .collect();
    emit_list(ctx.format, &summaries, &[], |_| Vec::new())?;
    Ok(())
}
