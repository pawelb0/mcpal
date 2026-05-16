use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::cli::DiffCategory;
use crate::runtime::Ctx;

pub async fn run(
    ref_a: &str,
    ref_b: &str,
    only: Option<DiffCategory>,
    ctx: &Ctx,
) -> Result<()> {
    let (a, b) = tokio::try_join!(load(ref_a, ctx), load(ref_b, ctx))?;
    let mut out = serde_json::Map::new();
    if only.is_none() || matches!(only, Some(DiffCategory::Tools)) {
        out.insert("tools".into(), serde_json::to_value(diff(&a.tools, &b.tools))?);
    }
    if only.is_none() || matches!(only, Some(DiffCategory::Resources)) {
        out.insert(
            "resources".into(),
            serde_json::to_value(diff(&a.resources, &b.resources))?,
        );
    }
    if only.is_none() || matches!(only, Some(DiffCategory::Prompts)) {
        out.insert("prompts".into(), serde_json::to_value(diff(&a.prompts, &b.prompts))?);
    }
    ctx.render_one(&Value::Object(out))?;
    Ok(())
}

struct Snapshot {
    tools: BTreeMap<String, Value>,
    resources: BTreeMap<String, Value>,
    prompts: BTreeMap<String, Value>,
}

async fn load(reference: &str, ctx: &Ctx) -> Result<Snapshot> {
    let (_, client) = ctx.open(reference).await?;
    let tools = ctx.under_deadline(client.list_all_tools()).await??;
    let resources = ctx.under_deadline(client.list_all_resources()).await??;
    let prompts = ctx.under_deadline(client.list_all_prompts()).await??;
    Ok(Snapshot {
        tools: tools
            .into_iter()
            .map(|t| (t.name.to_string(), serde_json::to_value(&t.input_schema).unwrap_or(Value::Null)))
            .collect(),
        resources: resources
            .into_iter()
            .map(|r| (r.uri.clone(), serde_json::to_value(&r).unwrap_or(Value::Null)))
            .collect(),
        prompts: prompts
            .into_iter()
            .map(|p| (p.name.clone(), serde_json::to_value(&p).unwrap_or(Value::Null)))
            .collect(),
    })
}

#[derive(Serialize, Default)]
struct CategoryDiff {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<String>,
}

fn diff(a: &BTreeMap<String, Value>, b: &BTreeMap<String, Value>) -> CategoryDiff {
    let mut out = CategoryDiff::default();
    for (name, va) in a {
        match b.get(name) {
            None => out.removed.push(name.clone()),
            Some(vb) if vb != va => out.changed.push(name.clone()),
            Some(_) => {}
        }
    }
    for name in b.keys() {
        if !a.contains_key(name) {
            out.added.push(name.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_added_removed_changed() {
        let mut a = BTreeMap::new();
        a.insert("foo".into(), json!({"x":1}));
        a.insert("baz".into(), json!({"x":1}));
        let mut b = BTreeMap::new();
        b.insert("foo".into(), json!({"x":2}));
        b.insert("bar".into(), json!({"x":1}));
        let d = diff(&a, &b);
        assert_eq!(d.added, vec!["bar"]);
        assert_eq!(d.removed, vec!["baz"]);
        assert_eq!(d.changed, vec!["foo"]);
    }
}
