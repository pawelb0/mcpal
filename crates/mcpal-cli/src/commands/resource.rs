use anyhow::Result;
use comfy_table::Table;
use mcpal_core::rmcp::model::ReadResourceRequestParams;
use mcpal_output::{Format, emit_json, emit_jsonl};

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

    match ctx.format {
        Format::Json => emit_json(&resources)?,
        Format::Jsonl => {
            for r in &resources {
                emit_jsonl(r)?;
            }
        }
        _ => {
            let mut table = Table::new();
            table.set_header(vec!["uri", "name", "mime"]);
            for r in &resources {
                table.add_row(vec![
                    r.uri.as_str(),
                    r.name.as_str(),
                    r.mime_type.as_deref().unwrap_or(""),
                ]);
            }
            println!("{table}");
        }
    }
    client.cancel().await.ok();
    Ok(())
}

async fn read(reference: &str, uri: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let result = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await?;

    match ctx.format {
        Format::Jsonl => emit_jsonl(&result)?,
        _ => emit_json(&result)?,
    }
    client.cancel().await.ok();
    Ok(())
}

async fn templates(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let templates = client.list_all_resource_templates().await?;

    match ctx.format {
        Format::Json => emit_json(&templates)?,
        Format::Jsonl => {
            for t in &templates {
                emit_jsonl(t)?;
            }
        }
        _ => {
            let mut table = Table::new();
            table.set_header(vec!["uri_template", "name", "mime"]);
            for t in &templates {
                table.add_row(vec![
                    t.uri_template.as_str(),
                    t.name.as_str(),
                    t.mime_type.as_deref().unwrap_or(""),
                ]);
            }
            println!("{table}");
        }
    }
    client.cancel().await.ok();
    Ok(())
}
