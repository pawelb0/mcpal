use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use mcpal_core::Client;
use mcpal_core::rmcp::model::{
    CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams, Tool,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde_json::Value;

use crate::kv;
use crate::runtime::{Ctx, probe};

const HELP: &str = "\
commands (AWS-CLI style: noun verb --flag value):
  tool list [json]                          compact list, or full JSON
  tool describe <name> [json]               schema + example, or full JSON
  tool call <name> [--key value ...]        call a tool with typed args
  resource list                             list resources
  resource read <uri>                       read a resource
  prompt list                               list prompts
  prompt get <name> [--key value ...]       fetch a prompt
  ping                                      server name + version
  help                                      this text
  quit | exit | Ctrl-D                      leave the repl
";

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (resolved, client) = ctx.open(reference).await?;
    let p = probe(&client);
    eprintln!(
        "mcpal repl @ {} ({} {})",
        resolved.display, p.name, p.version
    );
    eprintln!("type `help` for commands, `quit` to leave.");

    let history_path = history_file();
    let mut editor: Option<DefaultEditor> = Some(DefaultEditor::new().context("rustyline init")?);
    if let (Some(ed), Some(ref path)) = (editor.as_mut(), history_path.as_ref()) {
        let _ = ed.load_history(path);
    }

    loop {
        let mut taken = editor.take().expect("editor present");
        let (line, returned) = tokio::task::spawn_blocking(move || {
            let r = taken.readline("mcpal> ");
            if let Ok(ref l) = r {
                let _ = taken.add_history_entry(l);
            }
            (r, taken)
        })
        .await?;
        editor = Some(returned);

        match line {
            Ok(line) => {
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
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("readline: {e}");
                break;
            }
        }
    }

    if let (Some(ed), Some(ref path)) = (editor.as_mut(), history_path.as_ref())
        && let Some(parent) = path.parent()
    {
        let _ = std::fs::create_dir_all(parent);
        let _ = ed.save_history(path);
    }
    Ok(())
}

fn history_file() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    Some(base.data_dir().join("mcpal").join("repl_history"))
}

enum Control {
    Continue,
    Quit,
}

async fn dispatch(tokens: &[&str], client: &Client) -> Result<Control> {
    let head = tokens[0];
    let verb = tokens.get(1).copied();
    match (head, verb) {
        ("quit" | "exit", _) => Ok(Control::Quit),
        ("help" | "?", _) => {
            print!("{HELP}");
            Ok(Control::Continue)
        }
        ("ping", _) => {
            let p = probe(client);
            println!("{} {}", p.name, p.version);
            Ok(Control::Continue)
        }

        ("tool", Some("list")) => {
            let tools = client.list_all_tools().await?;
            if tokens.get(2) == Some(&"json") {
                print_json(&tools);
            } else {
                print_tools_brief(&tools);
            }
            Ok(Control::Continue)
        }
        ("tool", Some("describe")) => {
            let name = tokens
                .get(2)
                .ok_or_else(|| anyhow!("tool describe <name> [json]"))?;
            let tools = client.list_all_tools().await?;
            let tool = tools
                .iter()
                .find(|t| t.name == **name)
                .ok_or_else(|| anyhow!("no tool named '{name}'"))?;
            if tokens.get(3) == Some(&"json") {
                print_json(tool);
            } else {
                print_tool_detail(tool);
            }
            Ok(Control::Continue)
        }
        ("tool", Some("call")) => {
            let name = tokens
                .get(2)
                .ok_or_else(|| anyhow!("tool call <name> [--key value ...]"))?;
            let args = kv::parse_flag_args(tokens.get(3..).unwrap_or(&[]).iter().copied())?;
            let mut params = CallToolRequestParams::new(name.to_string());
            if !args.is_empty() {
                params = params.with_arguments(args);
            }
            let result = client.call_tool(params).await.context("tools/call")?;
            print_json(&result);
            Ok(Control::Continue)
        }

        ("resource", Some("list")) => {
            let resources = client.list_all_resources().await?;
            for r in &resources {
                println!("{}  {}", r.uri, r.name);
            }
            Ok(Control::Continue)
        }
        ("resource", Some("read")) => {
            let uri = tokens
                .get(2)
                .ok_or_else(|| anyhow!("resource read <uri>"))?;
            let result = client
                .read_resource(ReadResourceRequestParams::new((*uri).to_string()))
                .await?;
            print_json(&result);
            Ok(Control::Continue)
        }

        ("prompt", Some("list")) => {
            let prompts = client.list_all_prompts().await?;
            for p in &prompts {
                println!("{}  {}", p.name, first_line(p.description.as_deref()));
            }
            Ok(Control::Continue)
        }
        ("prompt", Some("get")) => {
            let name = tokens
                .get(2)
                .ok_or_else(|| anyhow!("prompt get <name> [--key value ...]"))?;
            let mut params = GetPromptRequestParams::new(name.to_string());
            if let Some(rest) = tokens.get(3..)
                && !rest.is_empty()
            {
                params = params.with_arguments(kv::parse_flag_args(rest.iter().copied())?);
            }
            let result = client.get_prompt(params).await?;
            print_json(&result);
            Ok(Control::Continue)
        }

        (noun @ ("tool" | "resource" | "prompt"), v) => {
            bail!(
                "unknown {noun} verb: {}; try `{noun} list`, `{noun} {}`",
                v.unwrap_or("(none)"),
                if noun == "tool" {
                    "call|describe"
                } else if noun == "resource" {
                    "read"
                } else {
                    "get"
                }
            )
        }
        (other, _) => Err(anyhow!("unknown command: {other}; try `help`")),
    }
}

fn print_json<T: serde::Serialize>(v: &T) {
    match serde_json::to_string_pretty(v) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("encode: {e}"),
    }
}

fn first_line(s: Option<&str>) -> &str {
    s.and_then(|s| s.lines().next()).unwrap_or("")
}

fn print_tools_brief(tools: &[Tool]) {
    let width = tools.iter().map(|t| t.name.len()).max().unwrap_or(0);
    for t in tools {
        println!(
            "{:<width$}  {}",
            t.name.as_ref(),
            first_line(t.description.as_deref()),
            width = width
        );
    }
}

fn print_tool_detail(tool: &Tool) {
    println!("{}", tool.name);
    if let Some(desc) = tool.description.as_deref() {
        for line in desc.lines() {
            println!("  {line}");
        }
        println!();
    }

    let schema = serde_json::to_value(&*tool.input_schema).unwrap_or(Value::Null);
    let (required, optional) = split_schema(&schema);

    if !required.is_empty() {
        println!("required:");
        for (name, ty) in &required {
            println!("  --{name} <{ty}>");
        }
    }
    if !optional.is_empty() {
        println!("optional:");
        for (name, ty) in &optional {
            println!("  --{name} <{ty}>");
        }
    }

    let example_args: Vec<String> = required
        .iter()
        .map(|(name, ty)| format!("--{name} {}", placeholder_for(ty)))
        .collect();
    if example_args.is_empty() {
        println!("\nexample:\n  tool call {}", tool.name);
    } else {
        println!(
            "\nexample:\n  tool call {} {}",
            tool.name,
            example_args.join(" ")
        );
    }
}

type Field = (String, String);

fn split_schema(schema: &Value) -> (Vec<Field>, Vec<Field>) {
    let required: std::collections::HashSet<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut req = Vec::new();
    let mut opt = Vec::new();
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (name, def) in props {
            let ty = def
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("any")
                .to_string();
            if required.contains(name) {
                req.push((name.clone(), ty));
            } else {
                opt.push((name.clone(), ty));
            }
        }
    }
    (req, opt)
}

fn placeholder_for(ty: &str) -> &'static str {
    match ty {
        "string" => "<text>",
        "number" | "integer" => "<n>",
        "boolean" => "<true|false>",
        "array" => "<json>",
        "object" => "<json>",
        _ => "<value>",
    }
}
