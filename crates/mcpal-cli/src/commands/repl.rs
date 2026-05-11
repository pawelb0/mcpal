use std::io::{IsTerminal, Write};

use anyhow::{Context, Result, anyhow};
use mcpal_core::Client;
use mcpal_core::rmcp::model::{
    CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams,
};
use serde_json::{Map, Value};

use crate::runtime::{Ctx, probe};

const HELP: &str = "\
commands:
  tools                       list available tools
  tool <name> [k=v ...]       call a tool
  resources                   list resources
  resource <uri>              read a resource
  prompts                     list prompts
  prompt <name> [k=v ...]     get a prompt
  ping                        show server name + version
  help                        this text
  quit | exit | Ctrl-D        leave the repl
";

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (resolved, client) = ctx.open(reference).await?;
    let tty = std::io::stdin().is_terminal();
    let p = probe(&client);
    if tty {
        eprintln!(
            "mcpal repl @ {} ({} {})",
            resolved.display, p.name, p.version
        );
        eprintln!("type `help` for commands, `quit` to leave.");
    }

    let mut reader = stdin_lines();
    loop {
        if tty {
            eprint!("mcpal> ");
            std::io::stderr().flush().ok();
        }
        let Some(line) = reader.next().await else {
            break;
        };
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        match dispatch(&tokens, &client).await {
            Ok(Control::Continue) => {}
            Ok(Control::Quit) => break,
            Err(e) => eprintln!("error: {e:#}"),
        }
    }
    Ok(())
}

enum Control {
    Continue,
    Quit,
}

async fn dispatch(tokens: &[&str], client: &Client) -> Result<Control> {
    match tokens[0] {
        "quit" | "exit" => Ok(Control::Quit),
        "help" | "?" => {
            print!("{HELP}");
            Ok(Control::Continue)
        }
        "ping" => {
            let p = probe(client);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "name": p.name, "version": p.version,
                }))?
            );
            Ok(Control::Continue)
        }
        "tools" => {
            let tools = client.list_all_tools().await?;
            print_json(&tools)
        }
        "tool" => {
            let name = tokens
                .get(1)
                .ok_or_else(|| anyhow!("tool <name> [k=v ...]"))?;
            let args = kv_pairs(&tokens[2..])?;
            let mut params = CallToolRequestParams::new(name.to_string());
            if !args.is_empty() {
                params = params.with_arguments(args);
            }
            let result = client.call_tool(params).await.context("tools/call")?;
            print_json(&result)
        }
        "resources" => {
            let resources = client.list_all_resources().await?;
            print_json(&resources)
        }
        "resource" => {
            let uri = tokens.get(1).ok_or_else(|| anyhow!("resource <uri>"))?;
            let result = client
                .read_resource(ReadResourceRequestParams::new((*uri).to_string()))
                .await?;
            print_json(&result)
        }
        "prompts" => {
            let prompts = client.list_all_prompts().await?;
            print_json(&prompts)
        }
        "prompt" => {
            let name = tokens
                .get(1)
                .ok_or_else(|| anyhow!("prompt <name> [k=v ...]"))?;
            let mut params = GetPromptRequestParams::new(name.to_string());
            let raw_args = &tokens[2..];
            if !raw_args.is_empty() {
                let mut map: Map<String, Value> = Map::new();
                for kv in raw_args {
                    let (k, v) = kv
                        .split_once('=')
                        .ok_or_else(|| anyhow!("expected k=v, got: {kv}"))?;
                    map.insert(k.into(), Value::String(v.into()));
                }
                params = params.with_arguments(map);
            }
            let result = client.get_prompt(params).await?;
            print_json(&result)
        }
        other => Err(anyhow!("unknown command: {other}; try `help`")),
    }
}

fn print_json<T: serde::Serialize>(v: &T) -> Result<Control> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(Control::Continue)
}

fn kv_pairs(tokens: &[&str]) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    for kv in tokens {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| anyhow!("expected k=v, got: {kv}"))?;
        let value: Value = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.into()));
        out.insert(k.into(), value);
    }
    Ok(out)
}

struct StdinLines {
    rx: tokio::sync::mpsc::Receiver<std::io::Result<String>>,
}

impl StdinLines {
    async fn next(&mut self) -> Option<std::io::Result<String>> {
        self.rx.recv().await
    }
}

fn stdin_lines() -> StdinLines {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<String>>(16);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut buf = String::new();
        loop {
            buf.clear();
            match std::io::BufRead::read_line(&mut stdin.lock(), &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if tx.blocking_send(Ok(buf.clone())).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(e));
                    break;
                }
            }
        }
    });
    StdinLines { rx }
}
