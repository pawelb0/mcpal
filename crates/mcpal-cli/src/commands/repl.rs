use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
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
commands:
  tools [json]                compact list (or full JSON dump)
  describe <name> [json]      tool description + example (or full JSON)
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
    match tokens[0] {
        "quit" | "exit" => Ok(Control::Quit),
        "help" | "?" => {
            print!("{HELP}");
            Ok(Control::Continue)
        }
        "ping" => {
            let p = probe(client);
            println!("{} {}", p.name, p.version);
            Ok(Control::Continue)
        }
        "tools" => {
            let tools = client.list_all_tools().await?;
            if tokens.get(1) == Some(&"json") {
                print_json(&tools);
            } else {
                print_tools_brief(&tools);
            }
            Ok(Control::Continue)
        }
        "describe" => {
            let name = tokens
                .get(1)
                .ok_or_else(|| anyhow!("describe <name> [json]"))?;
            let tools = client.list_all_tools().await?;
            let tool = tools
                .iter()
                .find(|t| t.name == **name)
                .ok_or_else(|| anyhow!("no tool named '{name}'"))?;
            if tokens.get(2) == Some(&"json") {
                print_json(tool);
            } else {
                print_tool_detail(tool);
            }
            Ok(Control::Continue)
        }
        "tool" => {
            let name = tokens
                .get(1)
                .ok_or_else(|| anyhow!("tool <name> [k=v ...]"))?;
            let args = kv::parse_pairs(tokens[2..].iter().copied(), "arg")?;
            let mut params = CallToolRequestParams::new(name.to_string());
            if !args.is_empty() {
                params = params.with_arguments(args);
            }
            let result = client.call_tool(params).await.context("tools/call")?;
            print_json(&result);
            Ok(Control::Continue)
        }
        "resources" => {
            let resources = client.list_all_resources().await?;
            for r in &resources {
                println!("{}  {}", r.uri, r.name);
            }
            Ok(Control::Continue)
        }
        "resource" => {
            let uri = tokens.get(1).ok_or_else(|| anyhow!("resource <uri>"))?;
            let result = client
                .read_resource(ReadResourceRequestParams::new((*uri).to_string()))
                .await?;
            print_json(&result);
            Ok(Control::Continue)
        }
        "prompts" => {
            let prompts = client.list_all_prompts().await?;
            for p in &prompts {
                println!("{}  {}", p.name, first_line(p.description.as_deref()));
            }
            Ok(Control::Continue)
        }
        "prompt" => {
            let name = tokens
                .get(1)
                .ok_or_else(|| anyhow!("prompt <name> [k=v ...]"))?;
            let mut params = GetPromptRequestParams::new(name.to_string());
            if tokens.len() > 2 {
                params =
                    params.with_arguments(kv::parse_pairs(tokens[2..].iter().copied(), "arg")?);
            }
            let result = client.get_prompt(params).await?;
            print_json(&result);
            Ok(Control::Continue)
        }
        other => Err(anyhow!("unknown command: {other}; try `help`")),
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
            println!("  {name}: {ty}");
        }
    }
    if !optional.is_empty() {
        println!("optional:");
        for (name, ty) in &optional {
            println!("  {name}: {ty}");
        }
    }

    let example_args: Vec<String> = required
        .iter()
        .map(|(name, ty)| format!("{name}={}", placeholder_for(ty)))
        .collect();
    if example_args.is_empty() {
        println!("\nexample:\n  tool {}", tool.name);
    } else {
        println!(
            "\nexample:\n  tool {} {}",
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
