use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use tokio::time::interval;

use crate::runtime::Ctx;
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

pub struct App {
    focus: Focus,
    quit: bool,
    sidebar: Sidebar,
}

impl App {
    pub fn new(ctx: &Ctx) -> Result<Self> {
        Ok(Self {
            focus: Focus::Sidebar,
            quit: false,
            sidebar: Sidebar::from_ctx(ctx)?,
        })
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut events = EventStream::new();
        let mut ticker = interval(Duration::from_millis(250));
        while !self.quit {
            terminal.draw(|f| self.render(f))?;
            tokio::select! {
                Some(Ok(ev)) = events.next() => self.on_event(ev),
                _ = ticker.tick() => {}
            }
        }
        Ok(())
    }

    fn on_event(&mut self, ev: Event) {
        let Event::Key(key) = ev else { return };
        if self.on_global(key) {
            return;
        }
        match self.focus {
            Focus::Sidebar => {
                self.sidebar.on_key(key);
            }
            Focus::Detail | Focus::Output => {}
        }
    }

    fn on_global(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Char('q')) => {
                self.quit = true;
                true
            }
            (_, KeyCode::Tab) => {
                self.focus = self.focus.next();
                true
            }
            (_, KeyCode::BackTab) => {
                self.focus = self.focus.prev();
                true
            }
            _ => false,
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
        f.render_widget(plain("Detail", self.focus == Focus::Detail), top[1]);
        f.render_widget(plain("Output", self.focus == Focus::Output), outer[1]);
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
