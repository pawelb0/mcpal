//! Classify a top-level error into a stable exit code + rustc-style hint
//! block. Each variant carries an `E####` code that `mcpal explain` can
//! expand into a longer note.

use mcpal_core::Error as CoreError;
use mcpal_output::Error as OutputError;

#[derive(Debug)]
pub struct Diagnostic {
    pub code: i32,
    pub error_code: &'static str,
    pub title: String,
    pub hints: Vec<&'static str>,
}

impl Diagnostic {
    fn build(
        code: i32,
        error_code: &'static str,
        title: impl Into<String>,
        hints: &[&'static str],
    ) -> Self {
        Self {
            code,
            error_code,
            title: title.into(),
            hints: hints.to_vec(),
        }
    }
}

pub fn classify(err: &anyhow::Error) -> Diagnostic {
    if let Some(OutputError::Query(msg)) = err.downcast_ref::<OutputError>() {
        return Diagnostic::build(
            2,
            "E0009",
            format!("query: {msg}"),
            &[
                "JMESPath syntax — see https://jmespath.org/tutorial.html",
                "common: `.field` projects, `[]` flattens, `[?cond]` filters",
                "preview without the filter to inspect the shape first",
            ],
        );
    }

    if let Some(core) = err.downcast_ref::<CoreError>() {
        return match core {
            CoreError::Io(e) => Diagnostic::build(
                6,
                "E0005",
                format!("transport: {e}"),
                &[
                    "check the URL is reachable, or the stdio command is on $PATH",
                    "run `mcpal server test <ref>` to retry just the handshake",
                ],
            ),
            CoreError::Auth(msg) => Diagnostic::build(
                4,
                "E0003",
                format!("auth required: {msg}"),
                &[
                    "run `mcpal auth login <ref> --bearer <TOKEN>` to store a token",
                    "or `mcpal auth login <ref> --oauth` for the OAuth 2.1 flow",
                ],
            ),
            CoreError::Service(msg) => {
                let lower = msg.to_lowercase();
                if lower.contains("unauthor") || lower.contains("401") {
                    Diagnostic::build(
                        5,
                        "E0004",
                        format!("auth expired: {msg}"),
                        &[
                            "run `mcpal auth refresh <ref>` to mint a new access token",
                            "or `mcpal auth login <ref> --oauth` to re-authorize from scratch",
                        ],
                    )
                } else {
                    Diagnostic::build(
                        7,
                        "E0006",
                        format!("server error: {msg}"),
                        &[
                            "check `mcpal --query 'serverInfo' server test <ref>`",
                            "try with `-v` for tracing output",
                        ],
                    )
                }
            }
            CoreError::NotFound(msg) => Diagnostic::build(
                3,
                "E0001",
                format!("not found: {msg}"),
                &[
                    "run `mcpal discover` to scan installed MCP clients for servers",
                    "or `mcpal server list --all` to see what's already configured",
                ],
            ),
            CoreError::Unsupported(what) => Diagnostic::build(
                6,
                "E0008",
                format!("not yet supported: {what}"),
                &["track progress in the upstream rmcp crate"],
            ),
        };
    }

    let s = format!("{err:#}").to_lowercase();
    if s.contains("timeout") || s.contains("timed out") {
        return Diagnostic::build(
            8,
            "E0007",
            err.to_string(),
            &[
                "retry, the server may have been slow",
                "for stdio servers, the initial `npx -y` install can take 30s+ on a cold cache",
            ],
        );
    }
    if s.contains("not found (owned, url, path, or discovered)")
        || s.contains("not found in mcpal config")
    {
        return Diagnostic::build(
            3,
            "E0001",
            err.to_string(),
            &[
                "run `mcpal discover` to scan installed MCP clients for servers",
                "or `mcpal server list --all` to see what's already configured",
                "or add one: `mcpal server add <alias> --stdio <command>`",
            ],
        );
    }
    if s.contains("expects k=v") || s.contains("expected --flag") {
        return Diagnostic::build(
            2,
            "E0002",
            err.to_string(),
            &[
                "use `--key value` pairs (AWS-CLI style): `mcpal tool call ev echo --message hi`",
                "for nested JSON, pass `--cli-input-json @file.json` or `--cli-input-json -`",
            ],
        );
    }
    if s.contains("parse json from") || s.contains("parse params as json") {
        return Diagnostic::build(
            2,
            "E0010",
            err.to_string(),
            &[
                "the payload isn't valid JSON; check for missing quotes or trailing commas",
                "`mcpal tool template <ref> <name>` prints a known-good skeleton you can pipe in",
                "for inline params, mind your shell's quoting: --params '{\"k\":\"v\"}'",
            ],
        );
    }
    Diagnostic::build(1, "E0000", err.to_string(), &[])
}

/// Render a clap parse error in rustc style. Returns the Diagnostic plus
/// the rendered clap usage block (so users still see the full help on
/// MissingSubcommand / DisplayHelp / DisplayVersion).
pub fn from_clap(err: &clap::Error) -> Option<(Diagnostic, String)> {
    use clap::error::ErrorKind as K;
    let kind = err.kind();
    let raw = err.render().to_string();
    let title = first_line(&raw).to_string();
    let (error_code, hints): (&'static str, &[&'static str]) = match kind {
        K::DisplayHelp | K::DisplayVersion | K::DisplayHelpOnMissingArgumentOrSubcommand => {
            return None;
        }
        K::UnknownArgument | K::InvalidSubcommand => (
            "E0002",
            &[
                "run `mcpal --help` (or `mcpal <subcommand> --help`) for the full grammar",
                "for arbitrary JSON-RPC methods use `mcpal raw <ref> <method> --params <...>`",
            ],
        ),
        K::MissingRequiredArgument | K::MissingSubcommand => (
            "E0002",
            &[
                "see the usage block above; positional args must come in order",
                "run `mcpal <subcommand> --help` to see the full grammar",
            ],
        ),
        K::InvalidValue | K::ValueValidation => (
            "E0002",
            &[
                "the value didn't match the expected format or enum",
                "see the `[possible values: …]` hint above",
            ],
        ),
        K::ArgumentConflict => (
            "E0002",
            &[
                "two mutually exclusive flags were both set",
                "drop one (e.g. `--stdio` vs `--http`, `--bearer` vs `--oauth`)",
            ],
        ),
        K::TooManyValues | K::TooFewValues | K::WrongNumberOfValues => (
            "E0002",
            &[
                "this flag takes a fixed number of values per occurrence",
                "repeat the flag for multiple values: `--arg foo --arg bar`",
            ],
        ),
        K::Io | K::Format => ("E0000", &[]),
        _ => ("E0002", &["run `mcpal --help`"]),
    };
    Some((Diagnostic::build(2, error_code, title, hints), raw))
}

fn first_line(s: &str) -> &str {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(s)
        .trim_start_matches("error: ")
}

/// Format a diagnostic in the rustc-style block:
///   error[E0001]: not found: server 'foo' …
///   help: run `mcpal discover`
///   help: …
///   For more information about this error, try `mcpal explain E0001`.
pub fn render(d: &Diagnostic) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out, "error[{}]: {}", d.error_code, d.title).ok();
    for hint in &d.hints {
        writeln!(out, "help: {hint}").ok();
    }
    if d.error_code != "E0000" {
        write!(
            out,
            "\nFor more information about this error, try `mcpal explain {}`.",
            d.error_code
        )
        .ok();
    }
    out
}

/// Long-form prose per error code. Mirrors `rustc --explain Exxxx`.
pub fn explain(code: &str) -> Option<&'static str> {
    let upper = code.to_ascii_uppercase();
    Some(match upper.as_str() {
        "E0000" => {
            "\
E0000 — generic error.

mcpal couldn't classify this failure into a known category, so the message
ends up here as a catch-all. The displayed text is whatever the underlying
library reported. If you can reproduce it, open an issue with the command,
the full message, and the `-v` trace output.\n"
        }

        "E0001" => {
            "\
E0001 — server reference not found.

mcpal didn't recognise the `<ref>` you passed. A reference resolves in this
order:

  1. a mcpal-owned alias (registered via `mcpal server add`)
  2. an `http://` or `https://` URL
  3. a path to a JSON file containing one ServerSpec
  4. a `<source>:<name>` from discovery (e.g. `cursor:linear`)
  5. a bare `<name>` if it's unambiguous across discovered sources

To fix:
  • `mcpal discover` — list everything installed clients already configured
  • `mcpal server list --all` — see mcpal-owned + discovered together
  • `mcpal server add <alias> --stdio <command>` — register a stdio server
  • `mcpal server add <alias> --http <url>` — register an HTTP server
\n"
        }

        "E0002" => {
            "\
E0002 — usage / invalid arguments.

mcpal couldn't parse the arguments you supplied. Most commonly this is a
malformed `--key value` flag pair.

To fix:
  • use AWS-CLI style flags: `mcpal tool call ev echo --message hi`
  • for nested JSON, use `--cli-input-json @args.json` (or `-` for stdin)
  • `mcpal tool template <ref> <name>` prints an example body you can pipe in
\n"
        }

        "E0003" => {
            "\
E0003 — auth required.

The server (or the tool/resource you're calling) needs credentials and none
are configured.

To fix:
  • bearer:  `mcpal auth login <ref> --bearer <TOKEN>`
  • OAuth:   `mcpal auth login <ref> --oauth`
  • one-shot env: `MCPAL_BEARER=… mcpal tool list <ref>`

Tokens persist in the OS keyring (Keychain on macOS, Secret Service on
Linux, Credential Manager on Windows). They never touch the TOML config.
\n"
        }

        "E0004" => {
            "\
E0004 — auth expired.

The server rejected the credentials mcpal sent. The access token has
likely expired.

To fix:
  • `mcpal auth refresh <ref>` — use the refresh token to mint a new one
  • `mcpal auth login <ref> --oauth` — full re-authorize when refresh fails
  • `mcpal auth status <ref>` — see what's currently stored
\n"
        }

        "E0005" => {
            "\
E0005 — transport error.

mcpal couldn't talk to the server. For stdio, the spawned process may have
failed to start; for HTTP, the URL may be wrong or unreachable.

To fix:
  • verify the URL with `curl -I <url>` (HEAD should return 200/4xx, not a
    network error)
  • for stdio: confirm the command is on $PATH and runs standalone
  • re-run with `-v` (or `-vv`) to see the underlying request
  • `mcpal server test <ref>` is the smallest reproducer
\n"
        }

        "E0006" => {
            "\
E0006 — server returned a JSON-RPC error.

mcpal got a well-formed response, but the server returned an error code
inside the JSON-RPC payload. Common causes:

  • the tool/resource/prompt doesn't exist on this server
  • the arguments don't match `inputSchema`
  • a server-side runtime failure

To fix:
  • `mcpal tool describe <ref> <name>` — confirm the input schema
  • `mcpal tool template <ref> <name>` — get a valid skeleton to fill in
  • re-run with `-v` for the raw JSON-RPC frame
\n"
        }

        "E0007" => {
            "\
E0007 — request timed out.

The server didn't respond within the deadline. For stdio servers the most
common cause is `npx -y @some-pkg` doing a fresh install (~30s on a cold
cache).

To fix:
  • simply retry; subsequent runs hit the npx cache
  • check the server isn't waiting on input (some stdio servers prompt
    interactively for config when first launched)
\n"
        }

        "E0008" => {
            "\
E0008 — not yet supported.

mcpal recognised the request but the underlying rmcp library (or mcpal
itself) doesn't implement the necessary plumbing yet.

To fix:
  • check `mcpal --version` and update if a newer release is out
  • for advanced flows, the `mcpal raw <ref> <method> --params …` escape
    hatch sends arbitrary JSON-RPC directly
\n"
        }

        _ => return None,
    })
}
