use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use mcpal_output::Format;

pub const EXIT_CODES_HELP: &str = "\
Exit codes:
  0    success
  1    generic error
  2    usage / invalid arguments
  3    server reference not found (try `mcpal discover` or `mcpal server list`)
  4    auth required (run `mcpal auth login <ref>` or `--oauth`)
  5    auth expired (run `mcpal auth refresh <ref>`)
  6    transport error / not yet supported
  7    server returned a JSON-RPC error
  8    request timed out (raise with `--timeout <SECS>`)
  130  interrupted by Ctrl-C
";

#[derive(Parser, Debug)]
#[command(
    name = "mcpal",
    version,
    about = "Scriptable command-line client for the Model Context Protocol",
    after_help = EXIT_CODES_HELP,
    long_about = "\
mcpal is a scriptable command-line client for the Model Context Protocol.

What it does:
  1. Reuses servers already configured by other clients. mcpal reads
     the MCP configs on disk from Claude Code, Claude Desktop, Cursor,
     Zed, opencode, LM Studio, Windsurf, and Cline.
  2. Speaks the full protocol — stdio + Streamable HTTP transports;
     tools, resources, resource templates, prompts, subscriptions,
     logging, server-initiated requests; bearer + OAuth 2.1 + PKCE +
     DCR; `raw` for any JSON-RPC method without a first-party verb.
  3. Works in pipelines — stable exit codes, `--output json|yaml`,
     `--query <jmespath>`, rustc-style errors with `E####` codes,
     `--timeout SECS`, Ctrl-C cancellation.

Common workflows:
  mcpal discover                        scan all clients for configured servers
  mcpal server list --all               mcpal-owned + discovered together
  mcpal server add <alias> -- <cmd>     register a stdio server
  mcpal server test <ref> [--full]      handshake + optional capability dump
  mcpal tool list <ref>                 compact list of tools on a server
  mcpal tool describe <ref> <name>      full schema for one tool
  mcpal tool call <ref> <name> [--key value ...]
  mcpal auth login <ref> --oauth        OAuth 2.1 + PKCE + DCR

`<ref>` accepts: mcpal-owned alias, <source>:<name> (from discovery),
bare <name> if unambiguous, raw https:// URL, or path to a JSON spec.

Default output is YAML; pass --output json for machine-readable JSON.\
"
)]
pub struct Cli {
    /// Reserved for future per-profile settings (currently a no-op).
    #[arg(long, global = true, env = "MCPAL_PROFILE", default_value = "default")]
    pub profile: String,

    /// Output format. Default is `yaml`; `json` is pretty-printed JSON.
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormat>,

    /// Path to the mcpal config file. Defaults to the OS config dir + `mcpal/config.toml`.
    #[arg(long, global = true, env = "MCPAL_CONFIG")]
    pub config: Option<PathBuf>,

    /// Repeat for more tracing. `-v` enables info-level mcpal logs; `-vv` is debug.
    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    pub verbosity: u8,

    /// Reserved — currently a no-op (no ANSI is emitted on stdout).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Skip interactive prompts (elicitation requests decline by default).
    #[arg(long, global = true)]
    pub no_interactive: bool,

    /// Filesystem root to expose to servers that call `roots/list` (repeatable).
    #[arg(long = "root", value_name = "PATH", global = true, num_args = 1)]
    pub roots: Vec<String>,

    /// Read a Claude/Cursor-style `mcp.json` and merge its servers into the
    /// session config without writing to disk. Useful for `mcpal --mcp-json
    /// ./mcp.json tool list <name>` against configs your team already has.
    #[arg(long, value_name = "PATH", global = true)]
    pub mcp_json: Option<PathBuf>,

    /// JMESPath expression applied to the response before output (AWS-CLI
    /// `--query` semantics). Drops everything that doesn't match. Example:
    /// `mcpal --query 'content[0].text' tool call ev echo --message hi`.
    #[arg(long, global = true, value_name = "JMESPATH")]
    pub query: Option<String>,

    /// Abort an in-flight request after N seconds. Without this flag mcpal
    /// waits indefinitely (servers can hang on cold `npx -y` installs).
    /// Ctrl-C aborts the wait at any time.
    #[arg(long, global = true, value_name = "SECS")]
    pub timeout: Option<u64>,

    /// External program that handles `sampling/createMessage` requests.
    /// mcpal pipes the request JSON on stdin and reads a CreateMessageResult
    /// JSON from stdout. Use your shell's quoting for multi-arg commands.
    #[arg(
        long = "sampling-handler",
        value_name = "CMD",
        global = true,
        env = "MCPAL_SAMPLING_HANDLER",
        allow_hyphen_values = true
    )]
    pub sampling_handler: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create an empty mcpal config file at the default path.
    ///
    /// Optional — `mcpal server add …` and the auth commands will create
    /// the file lazily. Run `init` if you want the file to exist (e.g. to
    /// open it in your editor via `mcpal config edit`) before adding any
    /// servers.
    Init,

    /// Inspect or edit the mcpal config file (path, contents, $EDITOR).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Add, list, show, remove, import, or smoke-test server entries.
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// List, describe, and call MCP tools on a server.
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },

    /// List resources and resource templates; read a resource by URI.
    Resource {
        #[command(subcommand)]
        action: ResourceAction,
    },

    /// List prompts and fetch one with `--key value` arguments.
    Prompt {
        #[command(subcommand)]
        action: PromptAction,
    },

    /// Send an arbitrary JSON-RPC request. Escape hatch for MCP methods
    /// without a dedicated subcommand.
    Raw {
        reference: String,
        method: String,
        /// Params payload: inline JSON, `@path/to/file.json`, or `-` for stdin.
        #[arg(long)]
        params: Option<String>,
    },

    /// Print shell completions for bash / zsh / fish.
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Scan installed MCP clients (Claude, Cursor, opencode, …) for servers
    /// they already configured. Servers found this way are usable directly
    /// as `<source>:<name>` without copying into mcpal config.
    Discover {
        /// Filter to a single source id (claude-code, cursor, …).
        #[arg(long)]
        source: Option<String>,
    },

    /// Store bearer tokens or run an OAuth 2.1 flow; tokens persist in the
    /// OS keyring (Keychain / Secret Service / Credential Manager).
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Tell the server which log level to emit via `logging/setLevel`.
    Logging {
        #[command(subcommand)]
        action: LoggingAction,
    },

    /// Open a session and tail every server-initiated notification
    /// (progress, log, resource-updated, list-changed) as YAML/JSON
    /// documents until Ctrl-C.
    Watch { reference: String },

    /// Explain an error code in long form (like `rustc --explain`).
    /// Example: `mcpal explain E0001`.
    Explain { code: String },

    /// Sanity-check mcpal's local environment: config, keyring, stored
    /// credentials per server, discovery sources. Pastable into a bug
    /// report (use `--output json`).
    Doctor,
}

#[derive(Subcommand, Debug)]
pub enum LoggingAction {
    /// Set the server-side logging verbosity. Level matches the MCP spec:
    /// debug | info | notice | warning | error | critical | alert | emergency.
    SetLevel {
        reference: String,
        #[arg(value_enum)]
        level: LogLevel,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl From<LogLevel> for mcpal_core::rmcp::model::LoggingLevel {
    fn from(l: LogLevel) -> Self {
        use mcpal_core::rmcp::model::LoggingLevel as L;
        match l {
            LogLevel::Debug => L::Debug,
            LogLevel::Info => L::Info,
            LogLevel::Notice => L::Notice,
            LogLevel::Warning => L::Warning,
            LogLevel::Error => L::Error,
            LogLevel::Critical => L::Critical,
            LogLevel::Alert => L::Alert,
            LogLevel::Emergency => L::Emergency,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Store a bearer token, or run the OAuth 2.1 authorize flow, for a
    /// server reference. Tokens persist in the OS keyring; bearer tokens
    /// can also be supplied at call time via `MCPAL_BEARER`.
    Login {
        reference: String,
        /// Bearer token. `-` reads stdin; omit to prompt interactively.
        #[arg(long, conflicts_with = "oauth")]
        bearer: Option<String>,
        /// Run the OAuth 2.1 authorization-code + PKCE flow against the server URL.
        #[arg(long)]
        oauth: bool,
        /// Server URL for OAuth discovery. Falls back to the resolved alias's URL.
        #[arg(long)]
        url: Option<String>,
        /// Don't open a browser automatically; just print the URL.
        #[arg(long)]
        no_browser: bool,
    },
    /// Forget the stored credentials for a reference (bearer or OAuth).
    Logout { reference: String },
    /// Show whether stored credentials exist.
    Status { reference: Option<String> },
    /// Use the stored refresh token to mint a new access token.
    Refresh {
        reference: String,
        /// Server URL for OAuth refresh. Falls back to the resolved alias's URL.
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print the active config file path (honors $MCPAL_CONFIG / --config).
    Path,
    /// Print the parsed config as TOML.
    Show,
    /// Open the config in $VISUAL or $EDITOR (falls back to `vi`).
    Edit,
}

#[derive(Subcommand, Debug)]
pub enum ServerAction {
    /// List mcpal-owned servers; `--all` includes discovered ones too.
    List(ServerListArgs),
    /// Print the full spec for one server (mcpal-owned or discovered).
    Show { reference: String },
    /// Register a new server in mcpal config (stdio or HTTP).
    Add(ServerAddArgs),
    /// Forget a mcpal-owned server (does not touch discovered entries).
    Remove { alias: String },
    /// Copy a discovered server into mcpal config so you can override env/auth/alias.
    Import(ServerImportArgs),
    /// Open + initialize a connection and print serverInfo. Acts as a
    /// liveness check (the MCP handshake fails if the server is broken).
    /// Pass `--full` to also list tool / resource / prompt counts and
    /// advertised capabilities.
    Test {
        reference: String,
        /// Enumerate tools, resources, prompts and report counts +
        /// advertised capabilities.
        #[arg(long)]
        full: bool,
    },
}

#[derive(clap::Args, Debug)]
pub struct ServerListArgs {
    /// Only show servers discovered from other clients.
    #[arg(long, conflicts_with = "all")]
    pub discovered: bool,
    /// Show both mcpal-owned and discovered servers.
    #[arg(long)]
    pub all: bool,
    /// Filter discovered rows to a single source id.
    #[arg(long)]
    pub source: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ServerImportArgs {
    /// Source id (claude-code, cursor, …).
    #[arg(long = "from")]
    pub from: String,
    /// Server name as exposed by the source.
    pub name: String,
    /// Alias to register in mcpal config (defaults to the source name).
    #[arg(long = "as")]
    pub alias: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ServerAddArgs {
    pub alias: String,

    /// Stdio command. Mutually exclusive with --http. Prefer the
    /// `mcpal server add <alias> -- <cmd> <args...>` form instead.
    #[arg(long, conflicts_with = "http")]
    pub stdio: Option<String>,

    /// Argument for the stdio command (repeatable). Prefer the trailing
    /// `-- <cmd> <args...>` form.
    #[arg(
        long = "arg",
        value_name = "ARG",
        num_args = 1,
        allow_hyphen_values = true
    )]
    pub args: Vec<String>,

    /// Environment variable in K=V form (repeatable).
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,

    /// HTTP URL. Mutually exclusive with --stdio.
    #[arg(long)]
    pub http: Option<String>,

    /// Stdio command + args after `--`. The first token is the program,
    /// the rest are its arguments. Example:
    /// `mcpal server add ev -- npx -y @modelcontextprotocol/server-everything`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub command: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum ToolAction {
    /// Compact list of tools on a server (name + description + required args).
    List { reference: String },

    /// Full schema for one tool (name, description, inputSchema, execution).
    Describe { reference: String, name: String },

    /// Print an example JSON argument body populated from the tool's
    /// inputSchema. Pipe into `tool call --cli-input-json -`.
    Template { reference: String, name: String },

    /// Invoke a tool with AWS-CLI style `--key value` flags.
    ///
    /// Values parse as typed JSON when possible (numbers, booleans, JSON
    /// literals); strings fall back to plain text. Pass `--cli-input-json
    /// <path|->` to read a base argument object from a file or stdin and
    /// override individual fields with extra `--key value` flags.
    Call {
        reference: String,
        name: String,
        /// JSON file (or `-` for stdin) used as the base argument object;
        /// `--key value` pairs override individual fields.
        #[arg(long, value_name = "PATH|-")]
        cli_input_json: Option<String>,
        /// Inline JSON body. Accepts `'{"k":"v"}'`, `@path`, or `-` for
        /// stdin. Mutually exclusive with `--cli-input-json`.
        #[arg(long, value_name = "JSON|@PATH|-", conflicts_with = "cli_input_json")]
        params: Option<String>,
        /// Remaining tokens are interpreted as `--key value` pairs.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResourceAction {
    /// List resources.
    List { reference: String },
    /// Read a resource by URI.
    Read { reference: String, uri: String },
    /// Subscribe to updates for one resource. Use `mcpal watch` (lands next)
    /// to actually stream the resulting notifications.
    Subscribe { reference: String, uri: String },
    /// Cancel a prior subscription.
    Unsubscribe { reference: String, uri: String },
    /// List resource templates.
    Template {
        #[command(subcommand)]
        action: ResourceTemplateAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResourceTemplateAction {
    List { reference: String },
}

#[derive(Subcommand, Debug)]
pub enum PromptAction {
    /// List prompts.
    List { reference: String },
    /// Get a prompt. Arguments use `--key value` pairs (AWS-CLI style).
    Get {
        reference: String,
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Yaml,
    Json,
}

impl From<OutputFormat> for Format {
    fn from(f: OutputFormat) -> Self {
        match f {
            OutputFormat::Yaml => Self::Yaml,
            OutputFormat::Json => Self::Json,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl From<Shell> for clap_complete::Shell {
    fn from(s: Shell) -> Self {
        match s {
            Shell::Bash => Self::Bash,
            Shell::Zsh => Self::Zsh,
            Shell::Fish => Self::Fish,
        }
    }
}
