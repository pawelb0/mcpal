use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value, to_value};

use crate::cli::DiffCategory;
use crate::runtime::Ctx;

pub async fn run(ref_a: &str, ref_b: &str, only: Option<DiffCategory>, ctx: &Ctx) -> Result<()> {
    let (a, b) = tokio::try_join!(load(ref_a, ctx), load(ref_b, ctx))?;
    let pick = |c: DiffCategory| only.is_none() || only == Some(c);
    let mut out = Map::new();
    if pick(DiffCategory::Tools) {
        out.insert("tools".into(), to_value(diff(&a.tools, &b.tools))?);
    }
    if pick(DiffCategory::Resources) {
        out.insert(
            "resources".into(),
            to_value(diff(&a.resources, &b.resources))?,
        );
    }
    if pick(DiffCategory::Prompts) {
        out.insert("prompts".into(), to_value(diff(&a.prompts, &b.prompts))?);
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
    let val = |v: serde_json::Result<Value>| v.unwrap_or(Value::Null);
    Ok(Snapshot {
        tools: tools
            .into_iter()
            .map(|t| (t.name.to_string(), val(to_value(&t.input_schema))))
            .collect(),
        resources: resources
            .into_iter()
            .map(|r| (r.uri.clone(), val(to_value(&r))))
            .collect(),
        prompts: prompts
            .into_iter()
            .map(|p| (p.name.clone(), val(to_value(&p))))
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
    let mut d = CategoryDiff::default();
    for (k, va) in a {
        match b.get(k) {
            None => d.removed.push(k.clone()),
            Some(vb) if vb != va => d.changed.push(k.clone()),
            _ => {}
        }
    }
    d.added = b.keys().filter(|k| !a.contains_key(*k)).cloned().collect();
    d
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
