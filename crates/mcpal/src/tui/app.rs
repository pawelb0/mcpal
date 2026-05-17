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
use crate::tui::detail::{self, Loaded, View};
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
    Bearer { reference: String, buf: String },
}

enum AsyncMsg {
    Connected {
        client: Client,
        loaded: Loaded,
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
                let level = n
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("info");
                let msg = n
                    .get("data")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                format!("→ log {level}: {msg}")
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
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
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
        let Some(modal) = self.modal.as_mut() else { return };
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

    fn on_sidebar_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter) {
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
            self.detail = match std::mem::replace(&mut self.detail, View::Empty) {
                View::Schema(_) => View::Empty,
                other => other,
            };
            return;
        }
        if matches!(key.code, KeyCode::Char('c'))
            && let Some(tool) = self.selected_tool().cloned()
        {
            self.modal = Some(Modal::Call(CallForm::new(tool)));
            return;
        }
        if matches!(key.code, KeyCode::Enter)
            && let Some(tool) = self.selected_tool().cloned()
        {
            self.detail = View::Schema(tool);
            return;
        }
        detail::on_key(&mut self.detail, key);
    }

    fn selected_tool(&self) -> Option<&mcpal_core::rmcp::model::Tool> {
        let View::Server { loaded, state, tab, .. } = &self.detail else {
            return None;
        };
        if !matches!(tab, detail::Tab::Tools) {
            return None;
        }
        loaded.tools.get(state.selected()?)
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
                    Ok((client, loaded)) => AsyncMsg::Connected { client, loaded },
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
        let Some(Modal::Call(form)) = self.modal.take() else { return };
        let tool_name = form.tool.name.to_string();
        let arguments = match form.to_json() {
            Ok(Value::Object(m)) => m,
            Ok(_) => return,
            Err(e) => {
                if let Some(loaded) = self.take_loaded() {
                    self.detail = View::CallResult {
                        loaded,
                        tool: tool_name,
                        outcome: Err(e),
                    };
                }
                return;
            }
        };
        let reference = match &self.detail {
            View::Server { loaded, .. } => loaded.reference.clone(),
            _ => return,
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
                let outcome = client
                    .call_tool(req)
                    .await
                    .map_err(|e| e.to_string());
                AsyncMsg::Called {
                    reference,
                    tool: tool_name,
                    outcome,
                }
            }
            .boxed(),
        );
    }

    fn take_loaded(&mut self) -> Option<Loaded> {
        match std::mem::replace(&mut self.detail, View::Empty) {
            View::Server { loaded, .. } => Some(loaded),
            View::CallResult { loaded, .. } => Some(loaded),
            other => {
                self.detail = other;
                None
            }
        }
    }

    fn on_async(&mut self, msg: AsyncMsg) {
        match msg {
            AsyncMsg::Connected { client, loaded } => {
                self.output.ok(format!(
                    "connected to {} ({} tools)",
                    loaded.reference,
                    loaded.tools.len()
                ));
                self.services
                    .insert(loaded.reference.clone(), Arc::new(client));
                self.detail = View::server(loaded);
                self.focus = Focus::Detail;
            }
            AsyncMsg::ConnectFailed { reference, err } => {
                self.output.err(format!("{reference}: {err}"));
                self.detail = View::Failed { reference, err };
            }
            AsyncMsg::Called { reference, tool, outcome } => {
                match &outcome {
                    Ok(r) if r.is_error.unwrap_or(false) => {
                        self.output.err(format!("{reference} {tool}: server error"));
                    }
                    Ok(_) => self.output.ok(format!("{reference} {tool}")),
                    Err(e) => self.output.err(format!("{reference} {tool}: {e}")),
                }
                let loaded = match self.take_loaded() {
                    Some(l) if l.reference == reference => l,
                    Some(l) => {
                        self.detail = View::Server {
                            loaded: l,
                            tab: detail::Tab::Tools,
                            state: Default::default(),
                        };
                        return;
                    }
                    None => return,
                };
                self.detail = View::CallResult { loaded, tool, outcome };
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
        self.sidebar
            .render(f, top[0], self.focus == Focus::Sidebar);
        detail::render(&mut self.detail, f, top[1], self.focus == Focus::Detail);
        self.output.render(f, outer[1], self.focus == Focus::Output);
        self.render_hint(f, outer[2]);
        match &self.modal {
            Some(Modal::Call(form)) => call::render(form, f, f.area()),
            Some(Modal::Bearer { reference, buf }) => render_bearer(reference, buf, f, f.area()),
            None => {}
        }
        if self.help {
            render_help(f, f.area());
        }
    }

    fn render_hint(&self, f: &mut Frame, area: Rect) {
        let text = match self.focus {
            Focus::Sidebar => "Tab cycle · Enter open · / filter · ? help · q quit",
            Focus::Detail => match &self.detail {
                View::Server { tab: detail::Tab::Tools, .. } => {
                    "Enter schema · c call · l next tab · Esc back · ? help"
                }
                View::Server { .. } => "l next tab · Esc back · ? help",
                View::Schema(_) | View::CallResult { .. } => "Esc back · ? help",
                _ => "? help · q quit",
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
                    Span::styled(
                        format!("  {:<14}", key),
                        Style::default().fg(Color::Cyan),
                    ),
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

