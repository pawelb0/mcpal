//! `key=value` argument parsing shared by tool/prompt/repl.

use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

pub fn parse_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.into()))
}

/// Walk `--key value` pairs (AWS-CLI style) into a typed JSON object.
/// Each token starting with `--` opens a new field; the very next token is
/// its value. Values parse via `parse_value` (typed JSON, string fallback).
pub fn parse_flag_args<I, S>(tokens: I) -> Result<Map<String, Value>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = Map::new();
    let mut iter = tokens.into_iter();
    while let Some(flag) = iter.next() {
        let flag = flag.as_ref().to_string();
        let name = flag
            .strip_prefix("--")
            .ok_or_else(|| anyhow!("expected --flag, got: {flag}"))?;
        let value = iter
            .next()
            .ok_or_else(|| anyhow!("--{name} requires a value"))?;
        out.insert(name.into(), parse_value(value.as_ref()));
    }
    Ok(out)
}

#[cfg(test)]
mod flag_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pairs() {
        let m = parse_flag_args(["--count", "3", "--msg", "hi", "--ok", "true"]).unwrap();
        assert_eq!(m["count"], json!(3));
        assert_eq!(m["msg"], json!("hi"));
        assert_eq!(m["ok"], json!(true));
    }

    #[test]
    fn missing_value_errors() {
        let err = parse_flag_args(["--count"]).unwrap_err();
        assert!(err.to_string().contains("requires a value"));
    }

    #[test]
    fn non_flag_token_errors() {
        let err = parse_flag_args(["count", "3"]).unwrap_err();
        assert!(err.to_string().contains("expected --flag"));
    }
}
