//! Classify a top-level error into a stable exit code + actionable hint.

use mcpal_core::Error as CoreError;

#[derive(Debug, Clone, Copy)]
pub struct Exit {
    pub code: i32,
    pub hint: Option<&'static str>,
}

impl Exit {
    const fn new(code: i32, hint: Option<&'static str>) -> Self {
        Self { code, hint }
    }
}

/// Walk an `anyhow::Error` chain and decide the exit code + hint. Codes
/// match the table printed by `mcpal --help`'s after_help block.
pub fn classify(err: &anyhow::Error) -> Exit {
    if let Some(core) = err.downcast_ref::<CoreError>() {
        return match core {
            CoreError::Io(_) => Exit::new(6, Some("server unreachable; check the URL or command")),
            CoreError::Auth(_) => Exit::new(4, Some("run `mcpal auth login <ref>` (or `--oauth`)")),
            CoreError::Service(msg) => {
                if msg.to_lowercase().contains("unauthor") || msg.to_lowercase().contains("401") {
                    Exit::new(5, Some("run `mcpal auth refresh <ref>` or re-login"))
                } else {
                    Exit::new(7, None)
                }
            }
            CoreError::NotFound(_) => {
                Exit::new(3, Some("try `mcpal discover` or `mcpal server list --all`"))
            }
            CoreError::Unsupported(_) => Exit::new(6, None),
        };
    }

    let s = format!("{err:#}").to_lowercase();
    if s.contains("timeout") || s.contains("timed out") {
        return Exit::new(8, Some("retry, or raise the timeout"));
    }
    if s.contains("not found (owned, url, path, or discovered)")
        || s.contains("not found in mcpal config")
    {
        return Exit::new(3, Some("try `mcpal discover` or `mcpal server list --all`"));
    }
    Exit::new(1, None)
}
