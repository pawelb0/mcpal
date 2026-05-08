mod cli;
mod commands;
mod config;
mod resolver;
mod runtime;

use anyhow::Result;
use clap::Parser;
use mcpal_output::Format;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
use config::Config;
use runtime::Ctx;

fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbosity);

    let code = match run(cli) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err:#}");
            err.downcast_ref::<mcpal_core::Error>()
                .map(mcpal_core::Error::exit_code)
                .unwrap_or(1)
        }
    };
    std::process::exit(code);
}

fn run(cli: Cli) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(dispatch(cli))
}

async fn dispatch(cli: Cli) -> Result<()> {
    let path = cli.config.unwrap_or_else(config::default_path);
    let cfg = Config::load(&path)?;
    let format = Format::resolve(cli.output.map(Into::into));
    let ctx = Ctx { cfg, format, config_path: path.clone() };

    match cli.command {
        Command::Init => commands::init::run(&path),
        Command::Config { action } => commands::config::run(action, &path),
        Command::Server { action } => commands::server::run(action, &ctx).await,
        Command::Tool { action } => commands::tool::run(action, &ctx).await,
        Command::Resource { action } => commands::resource::run(action, &ctx).await,
        Command::Prompt { action } => commands::prompt::run(action, &ctx).await,
        Command::Ping { reference } => commands::ping::run(&reference, &ctx).await,
        Command::Raw { reference, method, params } => {
            commands::raw::run(&reference, &method, params.as_deref(), &ctx).await
        }
        Command::Completion { shell } => commands::completion::run(shell),
    }
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info,mcpal=debug",
        _ => "debug",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
