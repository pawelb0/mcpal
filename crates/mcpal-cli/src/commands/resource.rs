use anyhow::Result;
use mcpal_core::rmcp::model::ReadResourceRequestParams;
use mcpal_output::{emit_list, emit_one};

use crate::cli::{ResourceAction, ResourceTemplateAction};
use crate::runtime::Ctx;

pub async fn run(action: ResourceAction, ctx: &Ctx) -> Result<()> {
    match action {
        ResourceAction::List { reference } => list(&reference, ctx).await,
        ResourceAction::Read { reference, uri } => read(&reference, &uri, ctx).await,
        ResourceAction::Template { action } => match action {
            ResourceTemplateAction::List { reference } => templates(&reference, ctx).await,
        },
    }
}

async fn list(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let resources = client.list_all_resources().await?;
    emit_list(ctx.format, &resources, &["uri", "name", "mime"], |r| {
        vec![
            r.uri.clone(),
            r.name.clone(),
            r.mime_type.clone().unwrap_or_default(),
        ]
    })?;
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

async fn templates(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let templates = client.list_all_resource_templates().await?;
    emit_list(
        ctx.format,
        &templates,
        &["uri_template", "name", "mime"],
        |t| {
            vec![
                t.uri_template.clone(),
                t.name.clone(),
                t.mime_type.clone().unwrap_or_default(),
            ]
        },
    )?;
    Ok(())
}
