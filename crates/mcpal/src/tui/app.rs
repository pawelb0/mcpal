use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::FutureExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use mcpal_core::Client;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use tokio::time::interval;

use crate::runtime::Ctx;
use crate::tui::detail::{self, Loaded, View};
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

enum ConnectionMsg {
    Loaded {
        client: Client,
        loaded: Loaded,
    },
    Failed {
        reference: String,
        err: String,
    },
}

pub struct App<'a> {
    ctx: &'a Ctx,
    focus: Focus,
    quit: bool,
    sidebar: Sidebar,
    detail: View,
    pending: Option<BoxFuture<'static, ConnectionMsg>>,
    services: HashMap<String, Arc<Client>>,
}

impl<'a> App<'a> {
    pub fn new(ctx: &'a Ctx) -> Result<Self> {
        Ok(Self {
            ctx,
            focus: Focus::Sidebar,
            quit: false,
            sidebar: Sidebar::from_ctx(ctx)?,
            detail: View::Empty,
            pending: None,
            services: HashMap::new(),
        })
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut events = EventStream::new();
        let mut ticker = interval(Duration::from_millis(250));
        while !self.quit {
            terminal.draw(|f| self.render(f))?;
            let pending = self.pending.as_mut();
            let pending_active = pending.is_some();
            tokio::select! {
                Some(Ok(ev)) = events.next() => self.on_event(ev),
                msg = poll_pending(pending), if pending_active => {
                    self.pending = None;
                    self.on_connection(msg);
                }
                _ = ticker.tick() => {}
            }
        }
        Ok(())
    }

    fn on_event(&mut self, ev: Event) {
        let Event::Key(key) = ev else { return };
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            self.quit = true;
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

    fn on_global(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => {
                self.quit = true;
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
        self.sidebar.on_key(key);
    }

    fn on_detail_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc) {
            self.detail = match std::mem::replace(&mut self.detail, View::Empty) {
                View::Schema(_) => {
                    // Re-fetch the last server view from services? For now
                    // just go empty; tab-back is rare.
                    View::Empty
                }
                other => other,
            };
            return;
        }
        if matches!(key.code, KeyCode::Enter)
            && let View::Server { loaded, state, tab, .. } = &self.detail
            && matches!(tab, detail::Tab::Tools)
            && let Some(i) = state.selected()
            && let Some(tool) = loaded.tools.get(i)
        {
            self.detail = View::Schema(tool.clone());
            return;
        }
        detail::on_key(&mut self.detail, key);
    }

    fn open_selected(&mut self) {
        let Some(entry) = self.sidebar.selected() else {
            return;
        };
        let reference = entry.display.clone();
        let spec = entry.spec.clone();
        let handler = self.ctx.handler.clone();
        self.detail = View::Connecting(reference.clone());
        self.pending = Some(
            async move {
                match detail::open(reference.clone(), spec, handler).await {
                    Ok((client, loaded)) => ConnectionMsg::Loaded { client, loaded },
                    Err(e) => ConnectionMsg::Failed {
                        reference,
                        err: e.to_string(),
                    },
                }
            }
            .boxed(),
        );
    }

    fn on_connection(&mut self, msg: ConnectionMsg) {
        match msg {
            ConnectionMsg::Loaded { client, loaded } => {
                self.services
                    .insert(loaded.reference.clone(), Arc::new(client));
                self.detail = View::server(loaded);
                self.focus = Focus::Detail;
            }
            ConnectionMsg::Failed { reference, err } => {
                self.detail = View::Failed { reference, err };
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)])
            .split(f.area());
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(0)])
            .split(outer[0]);
        self.sidebar
            .render(f, top[0], self.focus == Focus::Sidebar);
        detail::render(&mut self.detail, f, top[1], self.focus == Focus::Detail);
        f.render_widget(plain("Output", self.focus == Focus::Output), outer[1]);
    }
}

async fn poll_pending<'b>(
    fut: Option<&'b mut BoxFuture<'static, ConnectionMsg>>,
) -> ConnectionMsg {
    match fut {
        Some(f) => f.await,
        None => std::future::pending().await,
    }
}

fn plain(title: &str, focused: bool) -> Block<'_> {
    let style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(style)
}
