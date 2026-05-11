use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use mcpal_output::Format;

#[derive(Parser, Debug)]
#[command(name = "mcpal", version, about = "CLI for the Model Context Protocol")]
pub struct Cli {
    #[arg(long, global = true, env = "MCPAL_PROFILE", default_value = "default")]
    pub profile: String,

    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormat>,

    #[arg(long, global = true, env = "MCPAL_CONFIG")]
    pub config: Option<PathBuf>,

    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    pub verbosity: u8,

    #[arg(long, global = true)]
    pub no_color: bool,

    #[arg(long, global = true)]
    pub no_interactive: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Write a default config file.
    Init,

    /// Inspect or edit mcpal config.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage server entries.
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// Discover and call tools.
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },

    /// List and read resources.
    Resource {
        #[command(subcommand)]
        action: ResourceAction,
    },

    /// List and fetch prompts.
    Prompt {
        #[command(subcommand)]
        action: PromptAction,
    },

    /// Initialize a session and round-trip a ping.
    Ping {
        /// Server reference (alias, URL, or path to JSON spec).
        reference: String,
    },

    /// Send an arbitrary JSON-RPC request.
    Raw {
        reference: String,
        method: String,
        /// Inline params, `@file.json`, or `-` for stdin.
        #[arg(long)]
        params: Option<String>,
    },

    /// Generate shell completions.
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print the active config file path.
    Path,
    /// Print the config as TOML.
    Show,
    /// Open the config in `$EDITOR`.
    Edit,
}

#[derive(Subcommand, Debug)]
pub enum ServerAction {
    /// List configured + (later) discovered servers.
    List,
    /// Show details for one server.
    Show { reference: String },
    /// Add a new mcpal-owned server.
    Add(ServerAddArgs),
    /// Remove a mcpal-owned server.
    Remove { alias: String },
    /// Initialize against a server and ping it.
    Test { reference: String },
}

#[derive(clap::Args, Debug)]
pub struct ServerAddArgs {
    pub alias: String,

    /// Stdio command. Mutually exclusive with --http.
    #[arg(long, conflicts_with = "http")]
    pub stdio: Option<String>,

    /// Argument for the stdio command (repeatable).
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
}

#[derive(Subcommand, Debug)]
pub enum ToolAction {
    /// List tools exposed by a server.
    List { reference: String },

    /// Call a tool with named arguments.
    Call {
        reference: String,
        name: String,
        /// `key=value` (repeatable). Values parse as JSON when possible.
        #[arg(
            long = "arg",
            value_name = "K=V",
            num_args = 1,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
        /// JSON object file with arguments.
        #[arg(long)]
        args_file: Option<PathBuf>,
        /// Read JSON arguments from stdin.
        #[arg(long)]
        stdin_json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResourceAction {
    /// List resources.
    List { reference: String },
    /// Read a resource by URI.
    Read { reference: String, uri: String },
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
    /// Get a prompt with arguments.
    Get {
        reference: String,
        name: String,
        #[arg(long = "arg", value_name = "K=V", num_args = 1)]
        args: Vec<String>,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Jsonl,
    Yaml,
}

impl From<OutputFormat> for Format {
    fn from(f: OutputFormat) -> Self {
        match f {
            OutputFormat::Human => Self::Human,
            OutputFormat::Json => Self::Json,
            OutputFormat::Jsonl => Self::Jsonl,
            OutputFormat::Yaml => Self::Yaml,
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
