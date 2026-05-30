use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use mcpal_core::{AuthSpec, ServerSpec};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

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
    pub spec: ServerSpec,
}

pub struct Sidebar {
    entries: Vec<Entry>,
    state: ListState,
    filter: String,
    pub filter_active: bool,
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
                spec: spec.clone(),
            })
            .collect();
        for s in ctx.discovered()? {
            entries.push(Entry {
                display: format!("{}:{}", s.source, s.name),
                kind: Kind::from_spec(&s.spec),
                spec: s.spec.clone(),
            });
        }
        let mut state = ListState::default();
        if !entries.is_empty() {
            state.select(Some(0));
        }
        Ok(Self {
            entries,
            state,
            filter: String::new(),
            filter_active: false,
        })
    }

    fn visible(&self) -> Vec<&Entry> {
        filter_entries(&self.entries, &self.filter)
    }

    pub fn selected(&self) -> Option<&Entry> {
        let i = self.state.selected()?;
        self.visible().into_iter().nth(i)
    }

    fn clamp_selection(&mut self, len: usize) {
        if len == 0 {
            self.state.select(None);
        } else if self.state.selected().is_none_or(|i| i >= len) {
            self.state.select(Some(len - 1));
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        if self.filter_active {
            return self.on_filter_key(key);
        }
        let len = self.visible().len();
        if len == 0 && key.code != KeyCode::Char('/') {
            return false;
        }
        let cur = self.state.selected().unwrap_or(0);
        match key.code {
            KeyCode::Char('/') => {
                self.filter_active = true;
                true
            }
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

    fn on_filter_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.filter.clear();
                self.filter_active = false;
            }
            KeyCode::Enter => self.filter_active = false,
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Char(c) => self.filter.push(c),
            _ => return false,
        }
        let len = self.visible().len();
        self.clamp_selection(len);
        true
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        let show_filter = focused && (self.filter_active || !self.filter.is_empty());
        let (list_area, filter_area) = if show_filter {
            let v = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(area);
            (v[0], Some(v[1]))
        } else {
            (area, None)
        };
        let visible = self.visible();
        let items: Vec<ListItem> = visible
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
                    .title(format!(
                        "Servers ({}/{})",
                        visible.len(),
                        self.entries.len()
                    ))
                    .borders(Borders::ALL)
                    .border_style(border),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, list_area, &mut self.state);
        if let Some(rect) = filter_area {
            let prefix = if self.filter_active { "/" } else { " " };
            let cursor = if self.filter_active { "▌" } else { "" };
            let p = Paragraph::new(format!("{prefix}{}{cursor}", self.filter)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title("filter"),
            );
            f.render_widget(p, rect);
        }
    }
}

fn filter_entries<'a>(entries: &'a [Entry], filter: &str) -> Vec<&'a Entry> {
    if filter.is_empty() {
        entries.iter().collect()
    } else {
        entries
            .iter()
            .filter(|e| e.display.contains(filter))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(display: &str) -> Entry {
        Entry {
            display: display.into(),
            kind: Kind::Stdio,
            spec: ServerSpec::Stdio {
                command: "x".into(),
                args: vec![],
                env: Default::default(),
            },
        }
    }

    #[test]
    fn empty_filter_returns_all() {
        let xs = vec![e("a"), e("b"), e("c")];
        assert_eq!(filter_entries(&xs, "").len(), 3);
    }

    #[test]
    fn substring_filter_keeps_matches() {
        let xs = vec![e("cursor:linear"), e("zed:linear"), e("ev")];
        let kept: Vec<&str> = filter_entries(&xs, "linear")
            .iter()
            .map(|e| e.display.as_str())
            .collect();
        assert_eq!(kept, vec!["cursor:linear", "zed:linear"]);
    }

    #[test]
    fn filter_with_no_match_returns_empty() {
        let xs = vec![e("a"), e("b")];
        assert!(filter_entries(&xs, "zzz").is_empty());
    }

    #[test]
    fn filter_is_case_sensitive() {
        let xs = vec![e("Cursor"), e("cursor")];
        let kept: Vec<&str> = filter_entries(&xs, "Cur")
            .iter()
            .map(|e| e.display.as_str())
            .collect();
        assert_eq!(kept, vec!["Cursor"]);
    }
}
