use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use mcpal_core::{AuthSpec, ServerSpec};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::runtime::Ctx;

pub enum Kind {
    Stdio,
    Http,
    HttpOauth,
}

impl Kind {
    fn icon(&self) -> &'static str {
        match self {
            Kind::Stdio => "●",
            Kind::Http => "⚡",
            Kind::HttpOauth => "🔒",
        }
    }
    fn from_spec(spec: &ServerSpec) -> Self {
        match spec {
            ServerSpec::Stdio { .. } => Self::Stdio,
            ServerSpec::Http {
                auth: Some(AuthSpec::Oauth),
                ..
            } => Self::HttpOauth,
            ServerSpec::Http { .. } => Self::Http,
        }
    }
}

pub struct Entry {
    pub display: String,
    pub kind: Kind,
}

pub struct Sidebar {
    entries: Vec<Entry>,
    state: ListState,
}

impl Sidebar {
    pub fn from_ctx(ctx: &Ctx) -> Result<Self> {
        let mut entries: Vec<Entry> = ctx
            .cfg
            .server
            .iter()
            .map(|(alias, spec)| Entry {
                display: alias.clone(),
                kind: Kind::from_spec(spec),
            })
            .collect();
        for s in ctx.discovered()? {
            entries.push(Entry {
                display: format!("{}:{}", s.source, s.name),
                kind: Kind::from_spec(&s.spec),
            });
        }
        let mut state = ListState::default();
        if !entries.is_empty() {
            state.select(Some(0));
        }
        Ok(Self { entries, state })
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        let len = self.entries.len();
        if len == 0 {
            return false;
        }
        let cur = self.state.selected().unwrap_or(0);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down if cur + 1 < len => {
                self.state.select(Some(cur + 1));
                true
            }
            KeyCode::Char('k') | KeyCode::Up if cur > 0 => {
                self.state.select(Some(cur - 1));
                true
            }
            KeyCode::Char('g') => {
                self.state.select(Some(0));
                true
            }
            KeyCode::Char('G') => {
                self.state.select(Some(len - 1));
                true
            }
            _ => false,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|e| ListItem::new(format!("{} {}", e.kind.icon(), e.display)))
            .collect();
        let border = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!("Servers ({})", self.entries.len()))
                    .borders(Borders::ALL)
                    .border_style(border),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, area, &mut self.state);
    }
}
