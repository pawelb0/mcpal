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

    fn m(items: &[(&str, Value)]) -> BTreeMap<String, Value> {
        items.iter().map(|(k, v)| ((*k).into(), v.clone())).collect()
    }

    #[test]
    fn detects_added_removed_changed() {
        let a = m(&[("foo", json!({"x": 1})), ("baz", json!({"x": 1}))]);
        let b = m(&[("foo", json!({"x": 2})), ("bar", json!({"x": 1}))]);
        let d = diff(&a, &b);
        assert_eq!(d.added, vec!["bar"]);
        assert_eq!(d.removed, vec!["baz"]);
        assert_eq!(d.changed, vec!["foo"]);
    }

    #[test]
    fn identical_maps_diff_empty() {
        let a = m(&[("foo", json!(1)), ("bar", json!(2))]);
        let d = diff(&a, &a);
        assert!(d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty());
    }

    #[test]
    fn both_empty() {
        let d = diff(&BTreeMap::new(), &BTreeMap::new());
        assert!(d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty());
    }

    #[test]
    fn all_added_when_a_empty() {
        let b = m(&[("x", json!(1)), ("y", json!(2))]);
        let d = diff(&BTreeMap::new(), &b);
        assert_eq!(d.added, vec!["x", "y"]);
        assert!(d.removed.is_empty());
        assert!(d.changed.is_empty());
    }

    #[test]
    fn all_removed_when_b_empty() {
        let a = m(&[("x", json!(1)), ("y", json!(2))]);
        let d = diff(&a, &BTreeMap::new());
        assert_eq!(d.removed, vec!["x", "y"]);
        assert!(d.added.is_empty());
    }

    #[test]
    fn changed_detects_value_inequality_not_just_keys() {
        let a = m(&[("k", json!({"deep": [1, 2]}))]);
        let b = m(&[("k", json!({"deep": [1, 3]}))]);
        let d = diff(&a, &b);
        assert_eq!(d.changed, vec!["k"]);
    }
}
