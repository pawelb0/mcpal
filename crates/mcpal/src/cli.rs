use std::path::PathBuf;

use crate::output::Format;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use mcpal_core::rmcp::model::LoggingLevel;

#[derive(Parser, Debug)]
#[command(
    name = "mcpal",
    version,
    about = "Command-line client for the Model Context Protocol",
    long_about = "\
mcpal is a command-line client for the Model Context Protocol.

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
    #[arg(long, global = true, env = "MCPAL_PROFILE", default_value = "default")]
    pub profile: String,
    /// `yaml` (default) or `json`.
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormat>,
    #[arg(long, global = true, env = "MCPAL_CONFIG")]
    pub config: Option<PathBuf>,
    /// `-v` info; `-vv` debug.
    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    pub verbosity: u8,
    #[arg(long, global = true)]
    pub no_color: bool,
    /// Decline elicitation prompts.
    #[arg(long, global = true)]
    pub no_interactive: bool,
    /// Filesystem root for `roots/list` (repeatable).
    #[arg(long = "root", value_name = "PATH", global = true, num_args = 1)]
    pub roots: Vec<String>,
    /// Overlay a Claude/Cursor-style `mcp.json`.
    #[arg(long, value_name = "PATH", global = true)]
    pub mcp_json: Option<PathBuf>,
    /// Path to a collection file (`mcpal.yml`). Overrides walk-parents lookup.
    #[arg(long, global = true, value_name = "PATH")]
    pub collection: Option<PathBuf>,
    /// Additional `mcp.json` file to include in discovery (repeatable).
    #[arg(long = "discover-from", global = true, value_name = "PATH")]
    pub discover_from: Vec<PathBuf>,
    /// JMESPath filter applied to the response.
    #[arg(long, global = true, value_name = "JMESPATH")]
    pub query: Option<String>,
    /// Abort after N seconds.
    #[arg(long, global = true, value_name = "SECS")]
    pub timeout: Option<u64>,
    /// External `sampling/createMessage` handler (JSON stdin/stdout).
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
    /// Read / write the active config file.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage server entries and read protocol properties.
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// Invoke and inspect tools.
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },
    /// Read and subscribe to resources.
    Resource {
        #[command(subcommand)]
        action: ResourceAction,
    },
    /// Fetch prompts.
    Prompt {
        #[command(subcommand)]
        action: PromptAction,
    },
    /// Diff two servers' tools/resources/prompts.
    Diff {
        ref_a: String,
        ref_b: String,
        #[arg(long, value_enum)]
        only: Option<DiffCategory>,
    },
    /// Run a saved call from a collection file.
    #[command(after_help = "Examples:\n  \
        mcpal run get-issue --profile prod\n  \
        mcpal --collection ./mcpal.yml run echo --dry-run\n  \
        mcpal run echo --params-override message=override")]
    Run {
        name: String,
        /// Resolve + print the call without opening a connection.
        #[arg(long)]
        dry_run: bool,
        /// Overlay raw `K=V` params after templating (repeatable).
        #[arg(long = "params-override", value_name = "K=V", num_args = 1)]
        params_override: Vec<String>,
    },
    /// Send arbitrary JSON-RPC.
    #[command(after_help = "Examples:\n  \
        mcpal raw ev tools/list\n  \
        mcpal raw ev some/method --params '{\"k\":\"v\"}'\n  \
        mcpal raw ev some/method --params @payload.json\n  \
        cat payload.json | mcpal raw ev some/method --params -")]
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
    /// Bearer / OAuth 2.1 credentials (keyring).
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Set the server's log level.
    Logging {
        #[command(subcommand)]
        action: LoggingAction,
    },
    /// Tail server notifications until Ctrl-C.
    Watch { reference: String },
    /// Inspect mcp-ui and OpenAI Apps payloads in tool results.
    Ui {
        #[command(subcommand)]
        action: UiAction,
    },
    /// Local checks and error-code explanations.
    Debug {
        #[command(subcommand)]
        action: DebugAction,
    },
    /// Launch the interactive TUI.
    #[cfg(feature = "tui")]
    Tui,
}

#[derive(Subcommand, Debug)]
pub enum UiAction {
    /// Call a tool and classify each content block (mcp-ui, OpenAI Apps).
    Inspect {
        reference: String,
        name: String,
        /// Inline JSON, `@path`, or `-`. Pass tool arguments here.
        #[arg(long)]
        params: Option<String>,
        /// Write any UI/app resources to `/tmp/mcpal-ui-*` files.
        #[arg(long)]
        save: bool,
        /// Implies --save; also open each saved file with the OS opener.
        #[arg(long)]
        open: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DebugAction {
    /// Check local environment.
    Doctor,
    /// Print long-form prose for an `E####` code.
    Explain { code: String },
}

#[derive(Subcommand, Debug)]
pub enum LoggingAction {
    /// Set the server's emitted log level.
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

impl From<LogLevel> for LoggingLevel {
    fn from(l: LogLevel) -> Self {
        // Variant order matches LogLevel.
        [
            Self::Debug,
            Self::Info,
            Self::Notice,
            Self::Warning,
            Self::Error,
            Self::Critical,
            Self::Alert,
            Self::Emergency,
        ][l as usize]
    }
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Store a bearer or run the OAuth 2.1 flow.
    #[command(after_help = "Examples:\n  \
        mcpal auth login gh --bearer $GH_TOKEN\n  \
        echo $TOKEN | mcpal auth login gh --bearer -\n  \
        mcpal auth login notion --oauth\n\nMost users want `mcpal server add … --bearer` instead — this is the rotation entry-point.")]
    Login {
        reference: String,
        /// Bearer token; `-` reads stdin.
        #[arg(long, conflicts_with = "oauth")]
        bearer: Option<String>,
        /// Run OAuth 2.1 + PKCE.
        #[arg(long)]
        oauth: bool,
        /// Server URL (falls back to the resolved alias's URL).
        #[arg(long)]
        url: Option<String>,
        /// Print the authorize URL instead of opening a browser.
        #[arg(long)]
        no_browser: bool,
    },
    Logout {
        reference: String,
    },
    Status {
        reference: Option<String>,
    },
    /// Mint a new access token from the refresh token.
    Refresh {
        reference: String,
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Write the default config file.
    Init,
    /// Print the active config path.
    Path,
    /// Print the parsed config as TOML.
    Show,
    /// Open in $VISUAL / $EDITOR.
    Edit,
}

#[derive(Subcommand, Debug)]
pub enum ServerAction {
    List(ServerListArgs),
    Show {
        reference: String,
    },
    Add(ServerAddArgs),
    Remove {
        alias: String,
    },
    Import(ServerImportArgs),
    /// Print serverInfo.
    Info {
        reference: String,
    },
    /// Print the negotiated protocolVersion.
    Protocol {
        reference: String,
    },
    /// Print the advertised capability matrix.
    Capabilities {
        reference: String,
    },
    /// Print initialize-time instructions (or null).
    Instructions {
        reference: String,
    },
    /// Liveness check.
    Ping {
        reference: String,
    },
    /// Search the MCP Registry.
    Search {
        /// Named `keywords` to avoid collision with global `--query`.
        #[arg(value_name = "QUERY")]
        keywords: String,
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Install from the MCP Registry.
    Install(ServerInstallArgs),
    /// Scan installed MCP clients for already-configured servers.
    Discover {
        #[arg(long)]
        source: Option<String>,
    },
}

#[derive(clap::Args, Debug)]
pub struct ServerListArgs {
    /// Only entries registered via `server add` / `server import` — skip discovery.
    #[arg(long, conflicts_with_all = ["all", "discovered"])]
    pub owned: bool,
    /// Only entries from `server discover`.
    #[arg(long, conflicts_with = "all")]
    pub discovered: bool,
    /// Kept for back-compat — owned + discovered is now the default.
    #[arg(long, hide = true)]
    pub all: bool,
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
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
}

#[derive(clap::Args, Debug)]
#[command(
    after_help = "Examples:\n  \
        mcpal server add ev -- npx -y @modelcontextprotocol/server-everything\n  \
        mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer $GH_TOKEN\n  \
        mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer-env GH_TOKEN\n  \
        mcpal server add notion --http https://mcp.notion.com/v1 --oauth\n  \
        mcpal server add aws-api --env AWS_PROFILE=default -- uvx awslabs.aws-api-mcp-server@latest\n  \
        echo $TOKEN | mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer -",
    group(
        clap::ArgGroup::new("auth-mode")
            .args(["bearer", "bearer_env", "oauth"])
            .multiple(false)
            .required(false)
    ),
)]
pub struct ServerAddArgs {
    pub alias: String,
    #[arg(long, conflicts_with = "http")]
    pub stdio: Option<String>,
    #[arg(
        long = "arg",
        value_name = "ARG",
        num_args = 1,
        allow_hyphen_values = true
    )]
    pub args: Vec<String>,
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
    #[arg(long)]
    pub http: Option<String>,
    /// Literal token (or `-` for stdin) → OS keyring.
    #[arg(long, value_name = "TOKEN|-")]
    pub bearer: Option<String>,
    /// Spec auth = bearer_env { env = VAR } — token read from env at runtime.
    #[arg(long = "bearer-env", value_name = "VAR")]
    pub bearer_env: Option<String>,
    /// Run the OAuth 2.1 (PKCE + DCR) browser flow inline.
    #[arg(long)]
    pub oauth: bool,
    /// Pass `K: V` to the HTTP server (repeatable).
    #[arg(long = "header", value_name = "K: V", num_args = 1)]
    pub header: Vec<String>,
    /// With `--oauth`: write the spec but skip the browser handshake.
    #[arg(long = "no-login")]
    pub no_login: bool,
    /// Overwrite an existing entry of the same name.
    #[arg(long)]
    pub force: bool,
    /// `mcpal server add ev -- npx -y @mcp/server-everything`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub command: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum ToolAction {
    /// List tools.
    List {
        reference: String,
        /// One name per line.
        #[arg(long)]
        names_only: bool,
    },
    /// Print the full tool schema.
    Describe { reference: String, name: String },
    /// Print an example JSON body for the tool.
    Template { reference: String, name: String },
    /// Call a tool.
    #[command(after_help = "Examples:\n  \
        mcpal tool call ev echo --message hi\n  \
        mcpal tool call ev echo --params '{\"message\":\"hi\"}'\n  \
        echo '{\"message\":\"hi\"}' | mcpal tool call ev echo --params -\n  \
        mcpal tool call ev echo --cli-input-json @body.json\n  \
        mcpal --query 'content[0].text' tool call ev echo --message hi")]
    Call {
        reference: String,
        name: String,
        /// Base body from a file or `-` (stdin).
        #[arg(long, value_name = "PATH|-")]
        cli_input_json: Option<String>,
        /// Inline JSON, `@path`, or `-`.
        #[arg(long, value_name = "JSON|@PATH|-", conflicts_with = "cli_input_json")]
        params: Option<String>,
        /// Skip pre-send `inputSchema` check.
        #[arg(long)]
        skip_validation: bool,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResourceAction {
    List {
        reference: String,
        /// One URI per line.
        #[arg(long)]
        names_only: bool,
    },
    Read {
        reference: String,
        uri: String,
    },
    Subscribe {
        reference: String,
        uri: String,
    },
    Unsubscribe {
        reference: String,
        uri: String,
    },
    Template {
        #[command(subcommand)]
        action: ResourceTemplateAction,
    },
    /// `completion/complete` for a URI template argument.
    Complete {
        reference: String,
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
    List {
        reference: String,
        #[arg(long)]
        names_only: bool,
    },
    /// `--key value` pairs become prompt arguments.
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
