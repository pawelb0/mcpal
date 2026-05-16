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
    /// AWS-CLI JMESPath filter.
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
    /// Send arbitrary JSON-RPC.
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
    /// Local checks and error-code explanations.
    Debug {
        #[command(subcommand)]
        action: DebugAction,
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
    #[arg(long, conflicts_with = "all")]
    pub discovered: bool,
    /// Owned + discovered.
    #[arg(long)]
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
