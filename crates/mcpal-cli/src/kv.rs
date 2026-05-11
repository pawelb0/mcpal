//! `key=value` argument parsing shared by tool/prompt/repl.

use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

pub fn parse_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.into()))
}

pub fn parse_pairs<I, S>(pairs: I, flag: &str) -> Result<Map<String, Value>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = Map::new();
    for kv in pairs {
        let kv = kv.as_ref();
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| anyhow!("--{flag} expects K=V, got: {kv}"))?;
        out.insert(k.into(), parse_value(v));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typed_values() {
        let m = parse_pairs(["count=3", "msg=hi", "ok=true"], "arg").unwrap();
        assert_eq!(m["count"], json!(3));
        assert_eq!(m["msg"], json!("hi"));
        assert_eq!(m["ok"], json!(true));
    }

    #[test]
    fn missing_equals_errors() {
        let err = parse_pairs(["nope"], "arg").unwrap_err();
        assert!(err.to_string().contains("K=V"));
    }
}
