use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use mcpal_output::Format;

#[derive(Parser, Debug)]
#[command(
    name = "mcpal",
    version,
    about = "Scriptable command-line client for the Model Context Protocol",
    long_about = "\
Scriptable command-line client for the Model Context Protocol.

  mcpal server discover                 scan installed clients for servers
  mcpal server add <alias> -- <cmd>     register a stdio server
  mcpal server ping <ref>               liveness check
  mcpal tool list <ref> | call <ref> <name> [--key value …]
  mcpal auth login <ref> --oauth        OAuth 2.1 + PKCE + DCR

`<ref>` resolves as: alias → URL → JSON path → <source>:<name> → bare name.
Default output is YAML; pass --output json for machine-readable JSON.\
"
)]
pub struct Cli {
    /// Per-profile setting (currently unused).
    #[arg(long, global = true, env = "MCPAL_PROFILE", default_value = "default")]
    pub profile: String,

    /// `yaml` (default) or `json`.
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormat>,

    /// Config file path. Defaults to the OS config dir.
    #[arg(long, global = true, env = "MCPAL_CONFIG")]
    pub config: Option<PathBuf>,

    /// `-v` info-level mcpal logs; `-vv` debug.
    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    pub verbosity: u8,

    /// (reserved) Disable ANSI; mcpal already emits none on non-TTY stdout.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Decline interactive prompts (elicitation, etc.).
    #[arg(long, global = true)]
    pub no_interactive: bool,

    /// Filesystem root exposed via `roots/list` (repeatable).
    #[arg(long = "root", value_name = "PATH", global = true, num_args = 1)]
    pub roots: Vec<String>,

    /// Overlay a Claude/Cursor-style `mcp.json` into the session config.
    #[arg(long, value_name = "PATH", global = true)]
    pub mcp_json: Option<PathBuf>,

    /// AWS-CLI-style JMESPath filter applied to the response before output.
    #[arg(long, global = true, value_name = "JMESPATH")]
    pub query: Option<String>,

    /// Abort an in-flight request after N seconds (default: no deadline).
    #[arg(long, global = true, value_name = "SECS")]
    pub timeout: Option<u64>,

    /// External program for `sampling/createMessage` (JSON in/out on
    /// stdin/stdout).
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
    /// Inspect / edit mcpal's config (init, path, show, edit).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage server entries and read server properties.
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// `tools/*` — list, describe, template, call.
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },
    /// `resources/*` — list, read, templates, subscribe, complete.
    Resource {
        #[command(subcommand)]
        action: ResourceAction,
    },
    /// `prompts/*` — list, get, complete.
    Prompt {
        #[command(subcommand)]
        action: PromptAction,
    },
    /// Diff two servers' tools/resources/prompts.
    Diff {
        ref_a: String,
        ref_b: String,
        /// Limit the diff to one category.
        #[arg(long, value_enum)]
        only: Option<DiffCategory>,
    },
    /// Send arbitrary JSON-RPC. Escape hatch for unmapped methods.
    Raw {
        reference: String,
        method: String,
        /// Inline JSON, `@path`, or `-`.
        #[arg(long)]
        params: Option<String>,
    },
    /// Print shell completions.
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Bearer / OAuth 2.1 credential management (keyring-backed).
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// `logging/setLevel`.
    Logging {
        #[command(subcommand)]
        action: LoggingAction,
    },
    /// Tail server-initiated notifications until Ctrl-C.
    Watch { reference: String },
    /// `debug doctor` / `debug explain E####`.
    Debug {
        #[command(subcommand)]
        action: DebugAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum DebugAction {
    /// Check the local environment (config, keyring, auth, discovery).
    Doctor,
    /// Print the long-form prose for an `E####` code.
    Explain { code: String },
}

#[derive(Subcommand, Debug)]
pub enum LoggingAction {
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
    /// Store a bearer token or run the OAuth 2.1 flow.
    Login {
        reference: String,
        /// Bearer token; `-` reads stdin.
        #[arg(long, conflicts_with = "oauth")]
        bearer: Option<String>,
        /// Run OAuth 2.1 authorization-code + PKCE.
        #[arg(long)]
        oauth: bool,
        /// Server URL (falls back to the resolved alias's URL).
        #[arg(long)]
        url: Option<String>,
        /// Don't open a browser; print the URL.
        #[arg(long)]
        no_browser: bool,
    },
    /// Forget stored credentials.
    Logout { reference: String },
    /// Show whether stored credentials exist.
    Status { reference: Option<String> },
    /// Use the refresh token to mint a new access token.
    Refresh {
        reference: String,
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Create the config file at the default path.
    Init,
    /// Print the active config file path.
    Path,
    /// Print the parsed config as TOML.
    Show,
    /// Open the config in $VISUAL / $EDITOR (falls back to `vi`).
    Edit,
}

#[derive(Subcommand, Debug)]
pub enum ServerAction {
    /// List mcpal-owned servers; `--all` includes discovered ones.
    List(ServerListArgs),
    /// Print the full spec for one server.
    Show { reference: String },
    /// Register a server in mcpal config.
    Add(ServerAddArgs),
    /// Forget a mcpal-owned server.
    Remove { alias: String },
    /// Copy a discovered server into mcpal config.
    Import(ServerImportArgs),
    /// `serverInfo` (name, version, title).
    Info { reference: String },
    /// Negotiated MCP `protocolVersion`.
    Protocol { reference: String },
    /// Advertised capability matrix.
    Capabilities { reference: String },
    /// Free-form `instructions` string (or `null`).
    Instructions { reference: String },
    /// Liveness check (initialize handshake).
    Ping { reference: String },
    /// Search the MCP Registry.
    Search {
        /// Field is `keywords` (not `query`) to avoid collision with global `--query`.
        #[arg(value_name = "QUERY")]
        keywords: String,
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Install a server from the MCP Registry by name.
    Install(ServerInstallArgs),
    /// Scan installed MCP clients for already-configured servers.
    Discover {
        /// Filter to one source id.
        #[arg(long)]
        source: Option<String>,
    },
}

#[derive(clap::Args, Debug)]
pub struct ServerListArgs {
    /// Discovered-only.
    #[arg(long, conflicts_with = "all")]
    pub discovered: bool,
    /// Owned + discovered.
    #[arg(long)]
    pub all: bool,
    /// Filter discovered rows to one source id.
    #[arg(long)]
    pub source: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ServerImportArgs {
    #[arg(long = "from")]
    pub from: String,
    pub name: String,
    #[arg(long = "as")]
    pub alias: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ServerInstallArgs {
    /// e.g. `io.github.owner/repo`.
    pub name: String,
    #[arg(long = "as")]
    pub alias: Option<String>,
    /// Required env var(s) per the package's `environmentVariables`.
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct ServerAddArgs {
    pub alias: String,
    /// Prefer the trailing `-- <cmd> <args…>` form.
    #[arg(long, conflicts_with = "http")]
    pub stdio: Option<String>,
    /// Prefer the trailing `-- <cmd> <args…>` form.
    #[arg(long = "arg", value_name = "ARG", num_args = 1, allow_hyphen_values = true)]
    pub args: Vec<String>,
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
    /// Mutually exclusive with --stdio / trailing command.
    #[arg(long)]
    pub http: Option<String>,
    /// `mcpal server add ev -- npx -y @mcp/server-everything`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub command: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum ToolAction {
    /// Compact list of tools on a server (name + description + required args).
    List {
        reference: String,
        /// Print just the tool names, one per line. For shell completion.
        #[arg(long)]
        names_only: bool,
    },

    /// Full schema for one tool (name, description, inputSchema, execution).
    Describe { reference: String, name: String },

    /// Example JSON body populated from `inputSchema`.
    Template { reference: String, name: String },
    /// Call a tool with `--key value` flags (typed JSON when parseable).
    Call {
        reference: String,
        name: String,
        /// Base argument object from a file or `-` (stdin).
        #[arg(long, value_name = "PATH|-")]
        cli_input_json: Option<String>,
        /// Inline JSON, `@path`, or `-`. Conflicts with `--cli-input-json`.
        #[arg(long, value_name = "JSON|@PATH|-", conflicts_with = "cli_input_json")]
        params: Option<String>,
        /// Skip pre-send validation against `inputSchema`.
        #[arg(long)]
        skip_validation: bool,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResourceAction {
    /// List resources.
    List {
        reference: String,
        /// One URI per line.
        #[arg(long)]
        names_only: bool,
    },
    /// Read a resource by URI.
    Read { reference: String, uri: String },
    /// Subscribe to updates (combine with `mcpal watch`).
    Subscribe { reference: String, uri: String },
    /// Cancel a prior subscription.
    Unsubscribe { reference: String, uri: String },
    /// List resource templates.
    Template {
        #[command(subcommand)]
        action: ResourceTemplateAction,
    },
    /// `completion/complete` for a resource URI template argument.
    Complete {
        reference: String,
        /// URI template (e.g. `file:///{path}`).
        #[arg(long, value_name = "URI")]
        template: String,
        #[arg(long, value_name = "FIELD=PARTIAL")]
        arg: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResourceTemplateAction {
    List { reference: String },
}

#[derive(Subcommand, Debug)]
pub enum PromptAction {
    /// List prompts.
    List {
        reference: String,
        /// One name per line.
        #[arg(long)]
        names_only: bool,
    },
    /// Get a prompt with `--key value` arguments.
    Get {
        reference: String,
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
    /// `completion/complete` for a prompt argument.
    Complete {
        reference: String,
        name: String,
        #[arg(long, value_name = "FIELD=PARTIAL")]
        arg: String,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum DiffCategory {
    Tools,
    Resources,
    Prompts,
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
