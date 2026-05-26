use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::FutureExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use mcpal_core::Client;
use mcpal_core::rmcp::model::{CallToolRequestParams, CallToolResult};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::interval;

use crate::runtime::Ctx;
use crate::tui::call::{self, CallForm};
use crate::tui::detail::{self, Loaded, ServerView, View};
use crate::tui::output::OutputBuffer;
use crate::tui::sidebar::Sidebar;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Focus {
    Sidebar,
    Detail,
    Output,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::Detail,
            Self::Detail => Self::Output,
            Self::Output => Self::Sidebar,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Sidebar => Self::Output,
            Self::Detail => Self::Sidebar,
            Self::Output => Self::Detail,
        }
    }
}

enum Modal {
    Call(CallForm),
    Bearer {
        reference: String,
        buf: String,
    },
    EnvSetup {
        reference: String,
        spec: mcpal_core::ServerSpec,
        fields: Vec<(String, String)>,
        cursor: usize,
    },
}

enum AsyncMsg {
    Connected {
        client: Client,
        loaded: Loaded,
        warnings: Vec<String>,
    },
    ConnectFailed {
        reference: String,
        err: String,
    },
    Called {
        reference: String,
        tool: String,
        outcome: Result<CallToolResult, String>,
    },
}

pub struct App<'a> {
    ctx: &'a Ctx,
    focus: Focus,
    quit: bool,
    sidebar: Sidebar,
    detail: View,
    modal: Option<Modal>,
    pending: FuturesUnordered<BoxFuture<'static, AsyncMsg>>,
    services: HashMap<String, Arc<Client>>,
    output: OutputBuffer,
    notif_tx: UnboundedSender<Value>,
    notif_rx: UnboundedReceiver<Value>,
    help: bool,
}

impl<'a> App<'a> {
    pub fn new(ctx: &'a Ctx) -> Result<Self> {
        let (notif_tx, notif_rx) = unbounded_channel();
        Ok(Self {
            ctx,
            focus: Focus::Sidebar,
            quit: false,
            sidebar: Sidebar::from_ctx(ctx)?,
            detail: View::Empty,
            modal: None,
            pending: FuturesUnordered::new(),
            services: HashMap::new(),
            output: OutputBuffer::new(200),
            notif_tx,
            notif_rx,
            help: false,
        })
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut events = EventStream::new();
        let mut ticker = interval(Duration::from_millis(250));
        while !self.quit {
            terminal.draw(|f| self.render(f))?;
            tokio::select! {
                Some(Ok(ev)) = events.next() => self.on_event(ev),
                Some(msg) = self.pending.next(), if !self.pending.is_empty() => {
                    self.on_async(msg);
                }
                Some(notif) = self.notif_rx.recv() => self.on_notification(notif),
                _ = ticker.tick() => {}
            }
        }
        Ok(())
    }

    fn on_notification(&mut self, n: Value) {
        let kind = n.get("kind").and_then(|v| v.as_str()).unwrap_or("event");
        let summary = match kind {
            "progress" => format!(
                "→ progress {}/{}",
                n.get("progress")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into()),
                n.get("total")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into()),
            ),
            "log" => {
                let level = n.get("level").and_then(|v| v.as_str()).unwrap_or("info");
                let msg = n.get("data").map(|v| v.to_string()).unwrap_or_default();
                format!("→ log {level}: {msg}")
            }
            "elicitation_request" => {
                let msg = n
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no message)");
                format!("→ elicitation: {msg}")
            }
            "elicitation_response" => {
                let action = n.get("action").and_then(|v| v.as_str()).unwrap_or("?");
                format!("← elicit {action}")
            }
            "sampling_request" => {
                let n_msgs = n
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                format!("→ sampling: {n_msgs} message(s)")
            }
            "sampling_response" => {
                if let Some(err) = n.get("error").and_then(|v| v.as_str()) {
                    format!("← sampling error: {err}")
                } else {
                    let model = n.get("model").and_then(|v| v.as_str()).unwrap_or("?");
                    format!("← sampling reply from {model}")
                }
            }
            other => format!("→ {other}"),
        };
        self.output.info(summary);
    }

    fn on_event(&mut self, ev: Event) {
        let Event::Key(key) = ev else { return };
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                self.help = false;
            }
            return;
        }
        if self.modal.is_some() {
            self.on_modal_key(key);
            return;
        }
        let typing = self.focus == Focus::Sidebar && self.sidebar.filter_active;
        if !typing && self.on_global(key) {
            return;
        }
        match self.focus {
            Focus::Sidebar => self.on_sidebar_key(key),
            Focus::Detail => self.on_detail_key(key),
            Focus::Output => {}
        }
    }

    fn on_modal_key(&mut self, key: KeyEvent) {
        let Some(modal) = self.modal.as_mut() else {
            return;
        };
        match modal {
            Modal::Call(form) => match form.on_key(key) {
                call::Outcome::Cancel => self.modal = None,
                call::Outcome::Submit => self.submit_call(),
                call::Outcome::Handled => {}
            },
            Modal::Bearer { buf, .. } => match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Enter => self.save_bearer(),
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            },
            Modal::EnvSetup { fields, cursor, .. } => match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Tab | KeyCode::Down if !fields.is_empty() => {
                    *cursor = (*cursor + 1) % fields.len();
                }
                KeyCode::BackTab | KeyCode::Up if !fields.is_empty() => {
                    *cursor = if *cursor == 0 {
                        fields.len() - 1
                    } else {
                        *cursor - 1
                    };
                }
                KeyCode::Backspace => {
                    if let Some((_, val)) = fields.get_mut(*cursor) {
                        val.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some((_, val)) = fields.get_mut(*cursor) {
                        val.push(c);
                    }
                }
                KeyCode::Enter => self.save_env_setup(),
                _ => {}
            },
        }
    }

    fn save_bearer(&mut self) {
        let Some(Modal::Bearer { reference, buf }) = self.modal.take() else {
            return;
        };
        if buf.is_empty() {
            return;
        }
        match crate::keyring::put(&reference, crate::keyring::Kind::Bearer, &buf) {
            Ok(()) => self.output.ok(format!("bearer stored for {reference}")),
            Err(e) => self.output.err(format!("keyring: {e}")),
        }
    }

    fn save_env_setup(&mut self) {
        let Some(Modal::EnvSetup {
            reference,
            mut spec,
            fields,
            ..
        }) = self.modal.take()
        else {
            return;
        };
        // Patch the in-memory spec that will be passed to detail::open.
        if let mcpal_core::ServerSpec::Stdio { ref mut env, .. } = spec {
            for (k, v) in &fields {
                env.insert(k.clone(), v.clone());
            }
        }
        // Persist to disk so subsequent opens pick up the values.
        match crate::config::Config::load(&self.ctx.config_path) {
            Ok(mut cfg) => {
                if let Some(mcpal_core::ServerSpec::Stdio { env, .. }) =
                    cfg.server.get_mut(&reference)
                {
                    for (k, v) in &fields {
                        env.insert(k.clone(), v.clone());
                    }
                }
                if let Err(e) = cfg.save(&self.ctx.config_path) {
                    self.output.err(format!("env save: {e}"));
                }
            }
            Err(e) => self.output.err(format!("env save: load config: {e}")),
        }
        // Dispatch connect with the patched spec (bypasses stale sidebar entry).
        let mut handler = self.ctx.handler.clone();
        handler.events = Some(self.notif_tx.clone());
        self.output.info(format!("$ connect {reference}"));
        self.detail = crate::tui::detail::View::Connecting(reference.clone());
        self.pending.push(
            async move {
                match crate::tui::detail::open(reference.clone(), spec, handler).await {
                    Ok((client, loaded, warnings)) => AsyncMsg::Connected {
                        client,
                        loaded,
                        warnings,
                    },
                    Err(e) => AsyncMsg::ConnectFailed {
                        reference,
                        err: e.to_string(),
                    },
                }
            }
            .boxed(),
        );
    }

    fn on_global(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => {
                self.quit = true;
                true
            }
            KeyCode::Char('?') => {
                self.help = true;
                true
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
                true
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                true
            }
            _ => false,
        }
    }

    fn needs_env_setup(
        &self,
        reference: &str,
        spec: &mcpal_core::ServerSpec,
    ) -> Option<Vec<(String, String)>> {
        let mcpal_core::ServerSpec::Stdio { env, .. } = spec else {
            return None;
        };
        // Only ask for keys in `ctx.cfg` (not discovered entries) since we
        // need to write them back; fall back to the entry's own env map.
        let env_from_cfg = self
            .ctx
            .cfg
            .server
            .get(reference)
            .and_then(|s| {
                if let mcpal_core::ServerSpec::Stdio { env, .. } = s {
                    Some(env)
                } else {
                    None
                }
            })
            .unwrap_or(env);
        let empty: Vec<(String, String)> = env_from_cfg
            .iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| (k.clone(), String::new()))
            .collect();
        if empty.is_empty() { None } else { Some(empty) }
    }

    fn on_sidebar_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter) {
            if let Some(entry) = self.sidebar.selected() {
                let reference = entry.display.clone();
                let spec = entry.spec.clone();
                if let Some(fields) = self.needs_env_setup(&reference, &spec) {
                    self.modal = Some(Modal::EnvSetup {
                        reference,
                        spec,
                        fields,
                        cursor: 0,
                    });
                    return;
                }
            }
            self.open_selected();
            return;
        }
        if matches!(key.code, KeyCode::Char('b'))
            && let Some(entry) = self.sidebar.selected()
        {
            self.modal = Some(Modal::Bearer {
                reference: entry.display.clone(),
                buf: String::new(),
            });
            return;
        }
        self.sidebar.on_key(key);
    }

    fn on_detail_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc) {
            self.back_detail();
            return;
        }
        if matches!(key.code, KeyCode::Char('c'))
            && let Some(tool) = self.current_tool()
        {
            self.modal = Some(Modal::Call(CallForm::new(tool)));
            return;
        }
        if matches!(key.code, KeyCode::Enter) {
            self.drill_in();
            return;
        }
        if let View::Server(sv) = &mut self.detail {
            match key.code {
                KeyCode::Char('h') | KeyCode::Left => {
                    sv.tab = sv.tab.prev();
                    sv.reset_selection();
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    sv.tab = sv.tab.next();
                    sv.reset_selection();
                }
                KeyCode::Char('j') | KeyCode::Down => sv.move_by(1),
                KeyCode::Char('k') | KeyCode::Up => sv.move_by(-1),
                KeyCode::Char('g') => sv.top(),
                KeyCode::Char('G') => sv.bottom(),
                _ => {}
            }
        }
    }

    fn back_detail(&mut self) {
        let cur = std::mem::replace(&mut self.detail, View::Empty);
        self.detail = match cur {
            View::Schema { parent, .. } | View::CallResult { parent, .. } => View::Server(parent),
            View::Server(sv) => {
                self.focus = Focus::Sidebar;
                View::Server(sv)
            }
            View::Failed { .. } | View::Connecting(_) => {
                self.focus = Focus::Sidebar;
                View::Empty
            }
            View::Empty => View::Empty,
        };
    }

    fn drill_in(&mut self) {
        let cur = std::mem::replace(&mut self.detail, View::Empty);
        self.detail = match cur {
            View::Server(sv) if sv.tab == detail::Tab::Tools => match sv.selected_tool().cloned() {
                Some(tool) => View::Schema { parent: sv, tool },
                None => View::Server(sv),
            },
            other => other,
        };
    }

    fn current_tool(&self) -> Option<mcpal_core::rmcp::model::Tool> {
        match &self.detail {
            View::Server(sv) => sv.selected_tool().cloned(),
            View::Schema { tool, .. } => Some(tool.clone()),
            View::CallResult { parent, tool, .. } => parent
                .loaded
                .tools
                .iter()
                .find(|t| *t.name == *tool)
                .cloned(),
            _ => None,
        }
    }

    fn current_server_ref(&self) -> Option<String> {
        match &self.detail {
            View::Server(sv) => Some(sv.loaded.reference.clone()),
            View::Schema { parent, .. } | View::CallResult { parent, .. } => {
                Some(parent.loaded.reference.clone())
            }
            _ => None,
        }
    }

    fn set_call_result(&mut self, tool: String, outcome: Result<CallToolResult, String>) {
        let cur = std::mem::replace(&mut self.detail, View::Empty);
        let parent = match cur {
            View::Server(sv) => sv,
            View::Schema { parent, .. } | View::CallResult { parent, .. } => parent,
            other => {
                self.detail = other;
                return;
            }
        };
        self.detail = View::CallResult {
            parent,
            tool,
            outcome,
        };
    }

    fn open_selected(&mut self) {
        let Some(entry) = self.sidebar.selected() else {
            return;
        };
        let reference = entry.display.clone();
        let spec = entry.spec.clone();
        let mut handler = self.ctx.handler.clone();
        handler.events = Some(self.notif_tx.clone());
        self.output.info(format!("$ connect {reference}"));
        self.detail = View::Connecting(reference.clone());
        self.pending.push(
            async move {
                match detail::open(reference.clone(), spec, handler).await {
                    Ok((client, loaded, warnings)) => AsyncMsg::Connected {
                        client,
                        loaded,
                        warnings,
                    },
                    Err(e) => AsyncMsg::ConnectFailed {
                        reference,
                        err: e.to_string(),
                    },
                }
            }
            .boxed(),
        );
    }

    fn submit_call(&mut self) {
        let Some(Modal::Call(form)) = self.modal.take() else {
            return;
        };
        let tool_name = form.tool.name.to_string();
        let arguments = match form.to_json() {
            Ok(Value::Object(m)) => m,
            Ok(_) => return,
            Err(e) => {
                self.set_call_result(tool_name, Err(e));
                return;
            }
        };
        let Some(reference) = self.current_server_ref() else {
            return;
        };
        let Some(client) = self.services.get(&reference).cloned() else {
            return;
        };
        let mut req = CallToolRequestParams::new(tool_name.clone());
        if !arguments.is_empty() {
            req = req.with_arguments(arguments);
        }
        self.output
            .info(format!("$ tool call {reference} {tool_name}"));
        self.pending.push(
            async move {
                let outcome = client.call_tool(req).await.map_err(|e| e.to_string());
                AsyncMsg::Called {
                    reference,
                    tool: tool_name,
                    outcome,
                }
            }
            .boxed(),
        );
    }

    fn on_async(&mut self, msg: AsyncMsg) {
        match msg {
            AsyncMsg::Connected {
                client,
                loaded,
                warnings,
            } => {
                self.output.ok(format!(
                    "connected to {} ({} tools)",
                    loaded.reference,
                    loaded.tools.len()
                ));
                for w in warnings {
                    self.output.err(format!("{}: {}", loaded.reference, w));
                }
                self.services
                    .insert(loaded.reference.clone(), Arc::new(client));
                self.detail = View::Server(ServerView::new(loaded));
                self.focus = Focus::Detail;
            }
            AsyncMsg::ConnectFailed { reference, err } => {
                self.output.err(format!("{reference}: {err}"));
                self.detail = View::Failed { reference, err };
            }
            AsyncMsg::Called {
                reference,
                tool,
                outcome,
            } => {
                match &outcome {
                    Ok(r) if r.is_error.unwrap_or(false) => {
                        self.output.err(format!("{reference} {tool}: server error"));
                    }
                    Ok(_) => self.output.ok(format!("{reference} {tool}")),
                    Err(e) => self.output.err(format!("{reference} {tool}: {e}")),
                }
                if self.current_server_ref().as_deref() != Some(&reference) {
                    // User navigated to a different server; drop the late result.
                    return;
                }
                self.set_call_result(tool, outcome);
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(7),
                Constraint::Length(1),
            ])
            .split(f.area());
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(0)])
            .split(outer[0]);
        self.sidebar.render(f, top[0], self.focus == Focus::Sidebar);
        detail::render(&mut self.detail, f, top[1], self.focus == Focus::Detail);
        self.output.render(f, outer[1], self.focus == Focus::Output);
        self.render_hint(f, outer[2]);
        match &self.modal {
            Some(Modal::Call(form)) => call::render(form, f, f.area()),
            Some(Modal::Bearer { reference, buf }) => render_bearer(reference, buf, f, f.area()),
            Some(Modal::EnvSetup {
                reference,
                fields,
                cursor,
                ..
            }) => render_env_setup(reference, fields, *cursor, f, f.area()),
            None => {}
        }
        if self.help {
            render_help(f, f.area());
        }
    }

    fn render_hint(&self, f: &mut Frame, area: Rect) {
        let text = match self.focus {
            Focus::Sidebar => "Enter open · / filter · b bearer · Tab cycle · ? help · q quit",
            Focus::Detail => match &self.detail {
                View::Server(sv) if sv.tab == detail::Tab::Tools => {
                    "Enter schema · c call · h/l tab · Esc back · ? help"
                }
                View::Server(_) => "h/l tab · Esc back · ? help",
                View::Schema { .. } => "c call · Esc back · ? help",
                View::CallResult { .. } => "c call again · Esc back · ? help",
                _ => "Esc back · ? help",
            },
            Focus::Output => "PgUp/PgDn scroll · ? help · q quit",
        };
        f.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }
}

fn render_bearer(reference: &str, buf: &str, f: &mut Frame, area: Rect) {
    let popup = centered(area, 60, 20);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!("bearer for {reference}"));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let masked = "*".repeat(buf.len());
    f.render_widget(
        Paragraph::new(vec![
            Line::from(format!("› {masked}▌")),
            Line::from(""),
            Line::from("Enter save · Esc cancel"),
        ]),
        inner,
    );
}

fn render_env_setup(
    reference: &str,
    fields: &[(String, String)],
    cursor: usize,
    f: &mut Frame,
    area: Rect,
) {
    let popup = centered(area, 65, 60);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!("env setup for {reference}"));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from("Fill required environment variables:"));
    lines.push(Line::from(""));
    for (i, (name, val)) in fields.iter().enumerate() {
        let label_style = if i == cursor {
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(Span::styled(name.clone(), label_style)));
        let input = if i == cursor {
            format!("› {}▌", val)
        } else {
            format!("  {}", val)
        };
        lines.push(Line::from(input));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Tab next · Enter save+connect · Esc cancel",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_help(f: &mut Frame, area: Rect) {
    let popup = centered(area, 60, 70);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title("help (Esc to close)");
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let lines: Vec<Line<'static>> = HELP_LINES
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                Line::from("")
            } else if desc.is_empty() {
                Line::from(Span::styled(
                    key.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("  {:<14}", key), Style::default().fg(Color::Cyan)),
                    Span::raw(desc.to_string()),
                ])
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

const HELP_LINES: &[(&str, &str)] = &[
    ("NAVIGATION", ""),
    ("Tab / S-Tab", "cycle pane focus"),
    ("j/k  ↑/↓", "move selection"),
    ("gg / G", "top / bottom"),
    ("Enter", "drill in"),
    ("Esc", "drill back / close modal"),
    ("", ""),
    ("SERVERS", ""),
    ("/", "filter list"),
    ("", ""),
    ("TOOLS", ""),
    ("c", "call selected tool"),
    ("l / Right", "next tab"),
    ("", ""),
    ("GLOBAL", ""),
    ("q / Ctrl-C", "quit"),
    ("?", "toggle this help"),
];

fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
