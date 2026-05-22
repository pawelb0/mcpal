//! Classify an `anyhow::Error` into a stable exit code + `E####` block.
//! Long-form prose lives in the `EXPLAIN` table below (mirrored in the book).

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

/// Substring → (exit code, error code). First match wins. Patterns are
/// matched against the lowercased anyhow chain.
const ANYHOW_PATTERNS: &[(&str, i32, &str)] = &[
    ("' already exists", 2, "E0013"),
    ("interrupted by ctrl-c", 130, "E0011"),
    ("iserror: true", 7, "E0006"),
    ("schema validation", 2, "E0012"),
    ("timed out", 8, "E0007"),
    ("timeout", 8, "E0007"),
    ("not found (owned, url, path, or discovered)", 3, "E0001"),
    ("not found in mcpal config", 3, "E0001"),
    ("expects k=v", 2, "E0002"),
    ("expected --flag", 2, "E0002"),
    ("parse json from", 2, "E0010"),
    ("parse params as json", 2, "E0010"),
    ("auth flags require", 2, "E0002"),
];

pub fn classify(err: &anyhow::Error) -> Diagnostic {
    if let Some(OutputError::Query(msg)) = err.downcast_ref::<OutputError>() {
        return d(2, "E0009", format!("query: {msg}"));
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
    let lower = format!("{err:#}").to_lowercase();
    for (pat, code, ec) in ANYHOW_PATTERNS {
        if lower.contains(pat) {
            return d(*code, ec, err.to_string());
        }
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

/// Long-form prose per error code. Mirrors `rustc --explain Exxxx`.
const EXPLAIN: &[(&str, &str)] = &[
    (
        "E0000",
        "Generic error — mcpal couldn't classify it. Open an issue with the command \
        and `-v` trace.\n",
    ),
    (
        "E0001",
        "Server reference not found. A `<ref>` resolves as: owned alias → URL → \
        JSON file → `<source>:<name>` discovered → bare name (unambiguous). Fix with \
        `mcpal server discover`, `server list --all`, or `server add`.\n",
    ),
    (
        "E0002",
        "Bad arguments. Use AWS-CLI style `--key value`; for nested JSON pass \
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
];

pub fn explain(code: &str) -> Option<&'static str> {
    let upper = code.to_ascii_uppercase();
    EXPLAIN.iter().find(|(c, _)| *c == upper).map(|(_, p)| *p)
}
