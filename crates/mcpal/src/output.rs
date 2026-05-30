use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    #[default]
    Yaml,
    Json,
}

impl Format {
    pub fn resolve(explicit: Option<Format>) -> Self {
        explicit.unwrap_or(Self::Yaml)
    }
}

pub fn emit_one<T: Serialize>(format: Format, value: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    match format {
        Format::Json => {
            serde_json::to_writer_pretty(&mut out, value)?;
            out.write_all(b"\n")?;
        }
        Format::Yaml => serde_yaml::to_writer(&mut out, value)?,
    }
    Ok(())
}

pub fn apply_query(value: Value, query: Option<&str>) -> Result<Value, Error> {
    let Some(expr) = query else { return Ok(value) };
    let compiled = jmespath::compile(expr).map_err(|e| Error::Query(format!("compile: {e}")))?;
    let v =
        jmespath::Variable::from_serializable(&value).map_err(|e| Error::Query(e.to_string()))?;
    let r = compiled
        .search(v)
        .map_err(|e| Error::Query(format!("search: {e}")))?;
    serde_json::to_value(&*r).map_err(|e| Error::Query(e.to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error("jmespath {0}")]
    Query(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn q(value: Value, expr: &str) -> Value {
        apply_query(value, Some(expr)).expect("query")
    }

    #[test]
    fn none_query_returns_input() {
        let v = json!({"a": 1});
        assert_eq!(apply_query(v.clone(), None).unwrap(), v);
    }

    #[test]
    fn projects_field() {
        assert_eq!(q(json!({"a": {"b": 7}}), "a.b"), json!(7));
    }

    #[test]
    fn array_index() {
        assert_eq!(q(json!([10, 20, 30]), "[1]"), json!(20));
    }

    #[test]
    fn array_projection() {
        assert_eq!(
            q(json!([{"n": "x"}, {"n": "y"}]), "[].n"),
            json!(["x", "y"])
        );
    }

    #[test]
    fn missing_path_yields_null() {
        assert_eq!(q(json!({"a": 1}), "missing"), Value::Null);
    }

    #[test]
    fn filter_expression() {
        assert_eq!(
            q(json!([{"k": 1}, {"k": 2}, {"k": 3}]), "[?k > `1`].k"),
            json!([2, 3])
        );
    }

    #[test]
    fn length_function() {
        assert_eq!(q(json!([1, 2, 3, 4]), "length(@)"), json!(4));
    }

    #[test]
    fn malformed_expr_returns_query_error() {
        let err = apply_query(json!({}), Some("a..b")).unwrap_err();
        assert!(matches!(err, Error::Query(_)));
    }

    #[test]
    fn preserves_object_shape() {
        let v = json!({"a": {"nested": [1, 2]}, "b": "s"});
        assert_eq!(q(v.clone(), "@"), v);
    }
}
