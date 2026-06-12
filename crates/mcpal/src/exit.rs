//! Classify an `anyhow::Error` into a stable exit code + `E####` block.
//! Long-form prose lives in the `EXPLAIN` table below (mirrored in the book).

use crate::collection::template::TemplateError;
use crate::output::Error as OutputError;
use mcpal_core::Error as CoreError;

pub struct Diagnostic {
    pub code: i32,
    pub error_code: &'static str,
    pub title: String,
}

fn d(code: i32, ec: &'static str, title: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code,
        error_code: ec,
        title: title.into(),
    }
}

/// CLI-side errors with a fixed exit code + `E####` block. Anything raised
/// as plain `anyhow!` falls through to E0000/exit 1.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Usage(String),
    #[error("server returned tools/call result with isError: true")]
    ToolFailed,
    #[error("request timed out after {0}s")]
    Timeout(u64),
    #[error("{0}")]
    BadJson(String),
    #[error("interrupted by ctrl-c")]
    Interrupted,
    #[error("{0}")]
    Schema(String),
    #[error("server '{0}' already exists")]
    AlreadyExists(String),
    #[error("collection not found: {0}")]
    CollectionNotFound(String),
    #[error("profile '{0}' not in collection")]
    UnknownProfile(String),
    #[error("registry server requires env vars: {0} — re-run on a TTY or pass --env VAR=…")]
    NeedsEnv(String),
}

impl CliError {
    fn codes(&self) -> (i32, &'static str) {
        match self {
            Self::NotFound(_) => (3, "E0001"),
            Self::Usage(_) => (2, "E0002"),
            Self::ToolFailed => (7, "E0006"),
            Self::Timeout(_) => (8, "E0007"),
            Self::BadJson(_) => (2, "E0010"),
            Self::Interrupted => (130, "E0011"),
            Self::Schema(_) => (2, "E0012"),
            Self::AlreadyExists(_) => (2, "E0013"),
            Self::CollectionNotFound(_) => (2, "E0015"),
            Self::UnknownProfile(_) => (2, "E0016"),
            Self::NeedsEnv(_) => (2, "E0017"),
        }
    }
}

pub fn classify(err: &anyhow::Error) -> Diagnostic {
    if let Some(OutputError::Query(msg)) = err.downcast_ref::<OutputError>() {
        return d(2, "E0009", format!("query: {msg}"));
    }
    if let Some(cli) = err.downcast_ref::<CliError>() {
        let (code, ec) = cli.codes();
        return d(code, ec, err.to_string());
    }
    if err.downcast_ref::<TemplateError>().is_some() {
        return d(2, "E0014", err.to_string());
    }
    if let Some(core) = err.downcast_ref::<CoreError>() {
        return match core {
            CoreError::Io(e) => d(6, "E0005", format!("transport: {e}")),
            CoreError::Auth(m) => d(4, "E0003", format!("auth required: {m}")),
            CoreError::Service(m) => {
                let lower = m.to_lowercase();
                if lower.contains("unauthor") || lower.contains("401") {
                    d(5, "E0004", format!("auth expired: {m}"))
                } else {
                    d(7, "E0006", format!("server error: {m}"))
                }
            }
            CoreError::NotFound(m) => d(3, "E0001", format!("not found: {m}")),
            CoreError::Unsupported(w) => d(6, "E0008", format!("not yet supported: {w}")),
        };
    }
    d(1, "E0000", err.to_string())
}

/// Render a clap parse error in rustc style. Returns the Diagnostic plus
/// the rendered clap usage block.
pub fn from_clap(err: &clap::Error) -> Option<(Diagnostic, String)> {
    use clap::error::ErrorKind as K;
    let raw = err.render().to_string();
    let title = first_line(&raw).to_string();
    let (code, ec) = match err.kind() {
        K::DisplayHelp | K::DisplayVersion | K::DisplayHelpOnMissingArgumentOrSubcommand => {
            return None;
        }
        K::Io | K::Format => (1, "E0000"),
        _ => (2, "E0002"),
    };
    Some((d(code, ec, title), raw))
}

fn first_line(s: &str) -> &str {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(s)
        .trim_start_matches("error: ")
}

pub fn render(d: &Diagnostic) -> String {
    if d.error_code == "E0000" {
        format!("error[{}]: {}\n", d.error_code, d.title)
    } else {
        format!(
            "error[{}]: {}\nFor more information about this error, try `mcpal debug explain {}`.",
            d.error_code, d.title, d.error_code,
        )
    }
}

/// Mirrors `rustc --explain Exxxx`.
const EXPLAIN: &[(&str, &str)] = &[
    (
        "E0000",
        "Generic error — mcpal couldn't classify it. Open an issue with the command \
        and `-v` trace.\n",
    ),
    (
        "E0001",
        "Server reference not found. A `<ref>` resolves as: owned alias → \
        `cmd:<stdio command>` ephemeral → URL → JSON file → `<source>:<name>` \
        discovered → bare name (unambiguous). Fix with `mcpal server discover`, \
        `server list --all`, or `server add`.\n",
    ),
    (
        "E0002",
        "Bad arguments. Pass `--key value` pairs; for nested JSON pass \
        `--cli-input-json @args.json` or `--params '{…}'`. `tool template <ref> <name>` \
        prints a skeleton.\n",
    ),
    (
        "E0003",
        "Auth required. `mcpal auth login <ref> --bearer <TOKEN>` or `--oauth`. \
        `MCPAL_BEARER=…` works as a one-shot. Tokens live in the OS keyring.\n",
    ),
    (
        "E0004",
        "Auth expired. `mcpal auth refresh <ref>`, or a full `mcpal auth login \
        <ref> --oauth` if refresh also fails. `mcpal auth status <ref>` shows state.\n",
    ),
    (
        "E0005",
        "Transport error. Verify the URL with `curl -I`, confirm a stdio command \
        runs standalone, and re-run with `-v`/`-vv`. `mcpal server ping <ref>` is the \
        smallest reproducer.\n",
    ),
    (
        "E0006",
        "Server returned a JSON-RPC error. Check the tool name and arguments with \
        `tool describe` / `tool template`; re-run with `-v` to see the raw frame.\n",
    ),
    (
        "E0007",
        "Request timed out. Retry (cold `npx -y` cache is ~30s). Raise the budget \
        with `--timeout <SECS>` (default: unlimited).\n",
    ),
    (
        "E0008",
        "Not yet supported in mcpal. Use `mcpal raw <ref> <method> --params <…>` \
        as an escape hatch.\n",
    ),
    (
        "E0009",
        "Bad JMESPath query. Run the command without `--query` to see the shape. \
        Tutorial: https://jmespath.org/tutorial.html.\n",
    ),
    (
        "E0010",
        "JSON didn't parse. Quote inline JSON for your shell, use `@path` for \
        files, or `-` for stdin. `tool template <ref> <name>` prints a known-good \
        skeleton.\n",
    ),
    (
        "E0011",
        "Interrupted by Ctrl-C. mcpal dropped the in-flight request (exit 130). \
        The server may still complete it. For a hard deadline use `--timeout <SECS>`.\n",
    ),
    (
        "E0012",
        "Schema validation failed. `tool describe <ref> <name>` shows the schema; \
        `tool template` prints a skeleton. `--skip-validation` bypasses the check.\n",
    ),
    (
        "E0013",
        "Server name already registered. Run `mcpal server list` to see what \
        you have, or re-run with `--force` to overwrite. `mcpal server remove \
        <name>` deletes the entry first.\n",
    ),
    (
        "E0014",
        "Template variable not set. `mcpal.yml` references `{{profile.X}}` or \
        `{{env.X}}` that didn't resolve. Add the key to the active profile, set \
        the env var, or pass `--params-override KEY=VAL` to bypass.\n",
    ),
    (
        "E0015",
        "Collection not found. `mcpal run` looked for `mcpal.yml` in the current \
        directory and every parent, found none. Create one at your project root \
        or pass `--collection PATH` to point at a specific file.\n",
    ),
    (
        "E0016",
        "Active profile isn't declared in the collection. Either add a `profiles.<name>:` \
        block to `mcpal.yml`, pick a different `--profile`, or set `MCPAL_PROFILE`. \
        `default-profile:` at the top of `mcpal.yml` sets the fallback.\n",
    ),
    (
        "E0017",
        "Registry server declares required environment variables that aren't set. \
        Re-run `mcpal server install <ref>` on a TTY (mcpal will prompt) or pre-supply \
        each via `--env VAR=value`. `mcpal server search <ref>` shows the entry.\n",
    ),
];

pub fn explain(code: &str) -> Option<&'static str> {
    let upper = code.to_ascii_uppercase();
    EXPLAIN.iter().find(|(c, _)| *c == upper).map(|(_, p)| *p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    fn classify_cli(e: CliError) -> Diagnostic {
        classify(&anyhow::Error::new(e))
    }

    #[test]
    fn explain_codes_have_an_entry_per_variant() {
        // Every E#### the classifier can emit must have an EXPLAIN entry.
        let variants = [
            CliError::NotFound(String::new()),
            CliError::Usage(String::new()),
            CliError::ToolFailed,
            CliError::Timeout(0),
            CliError::BadJson(String::new()),
            CliError::Interrupted,
            CliError::Schema(String::new()),
            CliError::AlreadyExists(String::new()),
            CliError::CollectionNotFound(String::new()),
            CliError::UnknownProfile(String::new()),
            CliError::NeedsEnv(String::new()),
        ];
        for v in variants {
            let (_, ec) = v.codes();
            assert!(
                explain(ec).is_some(),
                "no EXPLAIN entry for classifier code {ec}"
            );
        }
    }

    #[test]
    fn timeout_maps_to_e0007_exit_8() {
        let d = classify_cli(CliError::Timeout(5));
        assert_eq!(d.error_code, "E0007");
        assert_eq!(d.code, 8);
        assert_eq!(d.title, "request timed out after 5s");
    }

    #[test]
    fn ctrl_c_maps_to_e0011_exit_130() {
        let d = classify_cli(CliError::Interrupted);
        assert_eq!(d.error_code, "E0011");
        assert_eq!(d.code, 130);
    }

    #[test]
    fn missing_env_maps_to_e0017() {
        let d = classify_cli(CliError::NeedsEnv("API_KEY".into()));
        assert_eq!(d.error_code, "E0017");
        assert_eq!(d.code, 2);
    }

    #[test]
    fn duplicate_alias_maps_to_e0013() {
        let d = classify_cli(CliError::AlreadyExists("gh".into()));
        assert_eq!(d.error_code, "E0013");
        assert_eq!(d.title, "server 'gh' already exists");
    }

    #[test]
    fn plain_anyhow_falls_through_to_e0000() {
        let d = classify(&anyhow!("something nobody anticipated"));
        assert_eq!(d.error_code, "E0000");
        assert_eq!(d.code, 1);
    }

    #[test]
    fn explain_is_case_insensitive() {
        assert!(explain("E0001").is_some());
        assert!(explain("e0001").is_some());
        assert!(explain("E9999").is_none());
    }

    #[test]
    fn collection_not_found_maps_to_e0015() {
        let d = classify_cli(CliError::CollectionNotFound("no mcpal.yml".into()));
        assert_eq!(d.error_code, "E0015");
    }

    #[test]
    fn usage_maps_to_e0002() {
        let d = classify_cli(CliError::Usage("--auth: unknown mode 'magic'".into()));
        assert_eq!(d.error_code, "E0002");
        assert_eq!(d.code, 2);
    }

    #[test]
    fn cli_error_survives_context_wrapping() {
        let err = anyhow::Error::new(CliError::NotFound("server 'x' not found".into()))
            .context("resolving reference");
        let d = classify(&err);
        assert_eq!(d.error_code, "E0001");
        assert_eq!(d.code, 3);
    }

    #[test]
    fn template_unset_maps_to_e0014() {
        use crate::collection::template::{Miss, Ns, TemplateError};
        let err = anyhow::Error::new(TemplateError {
            misses: vec![Miss {
                ns: Ns::Profile,
                key: "foo".into(),
            }],
        });
        let d = classify(&err);
        assert_eq!(d.error_code, "E0014");
        assert_eq!(d.title, "template variable not set: profile.foo");
    }
}
