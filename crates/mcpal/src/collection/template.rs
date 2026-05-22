//! Substitute `{{profile.X}}` and `{{env.X}}` into a JSON Value tree.
//! `{{{{` is the literal-`{{` escape.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub enum Ns {
    Profile,
    Env,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Miss {
    pub ns: Ns,
    pub key: String,
}

#[derive(Debug)]
pub struct TemplateError {
    pub misses: Vec<Miss>,
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "template variable not set: ")?;
        for (i, m) in self.misses.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            let ns = match m.ns {
                Ns::Profile => "profile",
                Ns::Env => "env",
            };
            write!(f, "{}.{}", ns, m.key)?;
        }
        Ok(())
    }
}
impl std::error::Error for TemplateError {}

fn pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{\{\s*(profile|env)\.([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").unwrap())
}

pub fn render(value: &mut Value, profile: &BTreeMap<String, String>) -> Result<(), TemplateError> {
    let mut misses = Vec::new();
    walk(value, profile, &mut misses);
    if misses.is_empty() {
        Ok(())
    } else {
        Err(TemplateError { misses })
    }
}

fn walk(value: &mut Value, profile: &BTreeMap<String, String>, misses: &mut Vec<Miss>) {
    match value {
        Value::String(s) => {
            *s = render_string(s, profile, misses);
        }
        Value::Array(a) => {
            for v in a {
                walk(v, profile, misses);
            }
        }
        Value::Object(m) => {
            for (_k, v) in m.iter_mut() {
                walk(v, profile, misses);
            }
        }
        _ => {}
    }
}

fn render_string(
    input: &str,
    profile: &BTreeMap<String, String>,
    misses: &mut Vec<Miss>,
) -> String {
    // `{{{{` is the literal-`{{` escape. Split on it, render each piece,
    // re-join with `{{`. `}}` carries no special meaning in the grammar.
    let mut out = String::with_capacity(input.len());
    for (i, piece) in input.split("{{{{").enumerate() {
        if i > 0 {
            out.push_str("{{");
        }
        let mut cursor = 0;
        for m in pattern().captures_iter(piece) {
            let whole = m.get(0).unwrap();
            out.push_str(&piece[cursor..whole.start()]);
            let ns = match &m[1] {
                "profile" => Ns::Profile,
                "env" => Ns::Env,
                _ => unreachable!(),
            };
            let key = m[2].to_string();
            let resolved = match ns {
                Ns::Profile => profile.get(&key).cloned(),
                Ns::Env => std::env::var(&key).ok(),
            };
            match resolved {
                Some(v) => out.push_str(&v),
                None => misses.push(Miss { ns, key }),
            }
            cursor = whole.end();
        }
        out.push_str(&piece[cursor..]);
    }
    out
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile() -> BTreeMap<String, String> {
        BTreeMap::from_iter([
            ("issue_id".to_string(), "ENG-1".to_string()),
            ("workspace".to_string(), "my-team".to_string()),
        ])
    }

    #[test]
    fn profile_substitution() {
        let mut v = json!("{{profile.issue_id}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("ENG-1"));
    }

    #[test]
    fn env_substitution() {
        // SAFETY: tests are single-threaded inside a #[test] but env is process-wide;
        // use a key namespaced for this test to avoid races.
        unsafe {
            std::env::set_var("MCPAL_TPL_TEST_KEY", "hello");
        }
        let mut v = json!("{{env.MCPAL_TPL_TEST_KEY}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("hello"));
    }

    #[test]
    fn mixed_string() {
        unsafe {
            std::env::set_var("MCPAL_TPL_USER", "pawel");
        }
        let mut v = json!("user={{env.MCPAL_TPL_USER}} ws={{profile.workspace}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("user=pawel ws=my-team"));
    }

    #[test]
    fn recursive_object_and_array() {
        let mut v = json!({
            "id": "{{profile.issue_id}}",
            "list": ["{{profile.workspace}}", 42, true, "lit"]
        });
        render(&mut v, &profile()).unwrap();
        assert_eq!(
            v,
            json!({
                "id": "ENG-1",
                "list": ["my-team", 42, true, "lit"]
            })
        );
    }

    #[test]
    fn numbers_and_bools_untouched() {
        let mut v = json!({"n": 7, "b": true, "f": 1.5, "null": null});
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!({"n": 7, "b": true, "f": 1.5, "null": null}));
    }

    #[test]
    fn missing_var_collects_all() {
        let mut v = json!({
            "a": "{{profile.nope}}",
            "b": "{{env.MCPAL_TPL_DEFINITELY_NOT_SET}}",
            "c": "ok"
        });
        let err = render(&mut v, &profile()).unwrap_err();
        assert_eq!(err.misses.len(), 2);
        let msg = err.to_string();
        assert!(msg.contains("template variable not set"), "{msg}");
        assert!(msg.contains("profile.nope"), "{msg}");
        assert!(msg.contains("env.MCPAL_TPL_DEFINITELY_NOT_SET"), "{msg}");
    }

    #[test]
    fn escape_literal_braces() {
        let mut v = json!("{{{{not a template}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("{{not a template}}"));
    }
}
