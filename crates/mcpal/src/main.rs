mod cli;
mod collection;
mod commands;
mod config;
mod exit;
mod keyring;
mod kv;
mod mcp_json;
mod oauth;
mod output;
mod registry;
mod resolver;
mod runtime;
#[cfg(feature = "tui")]
mod tui;

use crate::output::Format;
use anyhow::Result;
use clap::Parser;
use mcpal_core::Handler;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
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
    let mut cfg = config::Config::load(&path)?;
    if let Some(mcp_json) = cli.mcp_json.as_deref() {
        let extra = mcp_json::load(mcp_json)?;
        cfg.server.extend(extra);
    }
    let format = Format::resolve(cli.output.map(Into::into));
    let handler = Handler {
        roots: cli.roots,
        interactive: !cli.no_interactive,
        sampling_handler: cli
            .sampling_handler
            .as_deref()
            .map(|s| s.split_whitespace().map(String::from).collect())
            .filter(|v: &Vec<String>| !v.is_empty()),
        events: None,
    };
    let ctx = Ctx::new(
        cfg,
        format,
        cli.query,
        cli.timeout,
        path,
        cli.collection.clone(),
        cli.profile.clone(),
        handler,
    );

    use Command::*;
    match cli.command {
        Config { action } => commands::config::run(action, &ctx.config_path),
        Server { action } => commands::server::run(action, &ctx).await,
        Tool { action } => commands::tool::run(action, &ctx).await,
        Resource { action } => commands::resource::run(action, &ctx).await,
        Prompt { action } => commands::prompt::run(action, &ctx).await,
        Raw {
            reference,
            method,
            params,
        } => commands::raw::run(&reference, &method, params.as_deref(), &ctx).await,
        Run { .. } => Err(anyhow::anyhow!("mcpal run: wiring lands in Task 6")),
        Completion { shell } => commands::completion::run(shell),
        Auth { action } => commands::auth::run(action, &ctx).await,
        Logging { action } => commands::logging::run(action, &ctx).await,
        Watch { reference } => commands::watch::run(&reference, &ctx).await,
        Ui { action } => commands::ui::run(action, &ctx).await,
        Debug { action } => match action {
            cli::DebugAction::Doctor => commands::doctor::run(&ctx),
            cli::DebugAction::Explain { code } => exit::explain(&code)
                .map(|t| print!("{t}"))
                .ok_or_else(|| anyhow::anyhow!("no documentation for error code '{code}'")),
        },
        Diff { ref_a, ref_b, only } => commands::diff::run(&ref_a, &ref_b, only, &ctx).await,
        #[cfg(feature = "tui")]
        Tui => tui::run(&ctx).await,
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
