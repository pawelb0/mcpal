use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct Collection {
    #[serde(default, rename = "default-profile")]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub calls: BTreeMap<String, Call>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Call {
    pub server: String,
    pub tool: String,
    #[serde(default)]
    pub params: Value,
}

impl Collection {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read collection: {}", path.display()))?;
        serde_yaml::from_str(&text)
            .with_context(|| format!("parse YAML/JSON from {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_collection_ok() {
        let c: Collection = serde_yaml::from_str("calls: {}\nprofiles: {}\n").unwrap();
        assert!(c.calls.is_empty());
        assert!(c.profiles.is_empty());
        assert_eq!(c.default_profile, None);
    }

    #[test]
    fn full_collection_round_trips() {
        let src = r#"
default-profile: dev
profiles:
  dev:
    issue_id: ENG-1
    workspace: my-team
  prod:
    issue_id: ENG-999
    workspace: my-team
calls:
  get-issue:
    server: cursor:linear
    tool: get-issue
    params:
      id: "{{profile.issue_id}}"
      workspace: "{{profile.workspace}}"
"#;
        let c: Collection = serde_yaml::from_str(src).unwrap();
        assert_eq!(c.default_profile.as_deref(), Some("dev"));
        assert_eq!(c.profiles.len(), 2);
        assert_eq!(c.profiles["prod"]["issue_id"], "ENG-999");
        let call = &c.calls["get-issue"];
        assert_eq!(call.server, "cursor:linear");
        assert_eq!(call.tool, "get-issue");
        let params = call.params.as_object().expect("params is object");
        assert_eq!(params["id"].as_str(), Some("{{profile.issue_id}}"));
    }

    #[test]
    fn unknown_top_level_key_rejected() {
        let src = "wat: 1\ncalls: {}\n";
        let err = serde_yaml::from_str::<Collection>(src).unwrap_err();
        assert!(err.to_string().contains("wat"), "{err}");
    }

    #[test]
    fn unknown_call_key_rejected() {
        let src = r#"
calls:
  x:
    server: ev
    tool: echo
    nope: 1
"#;
        let err = serde_yaml::from_str::<Collection>(src).unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
    }
}
