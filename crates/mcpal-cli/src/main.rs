mod cli;
mod commands;
mod config;
mod exit;
mod keyring;
mod kv;
mod oauth;
mod resolver;
mod runtime;

use anyhow::Result;
use clap::Parser;
use mcpal_core::HandlerOptions;
use mcpal_output::Format;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
use config::Config;
use runtime::Ctx;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => match exit::from_clap(&e) {
            Some((d, usage)) => {
                eprint!("{usage}");
                eprintln!("{}", exit::render(&d));
                std::process::exit(d.code);
            }
            None => e.exit(),
        },
    };
    init_tracing(cli.verbosity);

    let code = match run(cli) {
        Ok(()) => 0,
        Err(err) => {
            let d = exit::classify(&err);
            eprintln!("{}", exit::render(&d));
            d.code
        }
    };
    std::process::exit(code);
}

fn run(cli: Cli) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(dispatch(cli))
}

async fn dispatch(cli: Cli) -> Result<()> {
    let path = cli.config.unwrap_or_else(config::default_path);
    let cfg = Config::load(&path)?;
    let format = Format::resolve(cli.output.map(Into::into));
    let handler_opts = HandlerOptions {
        roots: cli.roots,
        interactive: !cli.no_interactive,
        sampling_handler: cli
            .sampling_handler
            .as_deref()
            .map(|s| s.split_whitespace().map(String::from).collect()),
        events: None,
    };
    let ctx = Ctx::new(cfg, format, cli.query, path, handler_opts);

    match cli.command {
        Command::Init => commands::init::run(&ctx.config_path),
        Command::Config { action } => commands::config::run(action, &ctx.config_path),
        Command::Server { action } => commands::server::run(action, &ctx).await,
        Command::Tool { action } => commands::tool::run(action, &ctx).await,
        Command::Resource { action } => commands::resource::run(action, &ctx).await,
        Command::Prompt { action } => commands::prompt::run(action, &ctx).await,
        Command::Raw {
            reference,
            method,
            params,
        } => commands::raw::run(&reference, &method, params.as_deref(), &ctx).await,
        Command::Completion { shell } => commands::completion::run(shell),
        Command::Discover { source } => commands::discover::run(source.as_deref(), &ctx),
        Command::Auth { action } => commands::auth::run(action, &ctx).await,
        Command::Logging { action } => commands::logging::run(action, &ctx).await,
        Command::Watch { reference } => commands::watch::run(&reference, &ctx).await,
        Command::Explain { code } => match exit::explain(&code) {
            Some(text) => {
                print!("{text}");
                Ok(())
            }
            None => anyhow::bail!("no documentation for error code '{code}'"),
        },
    }
}

fn init_tracing(verbosity: u8) {
    if verbosity == 0 && std::env::var_os("RUST_LOG").is_none() {
        return;
    }
    let level = if verbosity == 1 {
        "info,mcpal=debug"
    } else {
        "debug"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
