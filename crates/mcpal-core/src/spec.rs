use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum ServerSpec {
    Stdio {
        command: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth: Option<AuthSpec>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthSpec {
    Bearer { token: String },
    BearerEnv { env: String },
    Oauth,
}

impl ServerSpec {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Stdio { .. } => "stdio",
            Self::Http { .. } => "http",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_roundtrip() {
        let spec = ServerSpec::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-everything".into()],
            env: BTreeMap::from([("DEBUG".to_string(), "1".to_string())]),
        };
        let toml = toml::to_string(&spec).unwrap();
        let back: ServerSpec = toml::from_str(&toml).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn http_with_oauth() {
        let spec = ServerSpec::Http {
            url: "https://mcp.linear.app/sse".into(),
            headers: BTreeMap::new(),
            auth: Some(AuthSpec::Oauth),
        };
        let toml = toml::to_string(&spec).unwrap();
        assert!(toml.contains("transport = \"http\""));
        assert!(toml.contains("type = \"oauth\""));
        let back: ServerSpec = toml::from_str(&toml).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn stdio_minimal() {
        let toml = r#"
            transport = "stdio"
            command = "echo"
        "#;
        let spec: ServerSpec = toml::from_str(toml).unwrap();
        match spec {
            ServerSpec::Stdio { command, args, env } => {
                assert_eq!(command, "echo");
                assert!(args.is_empty());
                assert!(env.is_empty());
            }
            _ => panic!("expected stdio"),
        }
    }
}
