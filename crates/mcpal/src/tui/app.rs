use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use tokio::time::interval;

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
enum Focus {
    #[default]
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

#[derive(Default)]
pub struct App {
    focus: Focus,
    quit: bool,
}

impl App {
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
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Char('q')) => {
                self.quit = true;
            }
            (_, KeyCode::Tab) => self.focus = self.focus.next(),
            (_, KeyCode::BackTab) => self.focus = self.focus.prev(),
            _ => {}
        }
    }

    fn render(&self, f: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)])
            .split(f.area());
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(0)])
            .split(outer[0]);
        f.render_widget(self.pane_block("Servers", Focus::Sidebar), top[0]);
        f.render_widget(self.pane_block("Detail", Focus::Detail), top[1]);
        f.render_widget(self.pane_block("Output", Focus::Output), outer[1]);
    }

    fn pane_block<'a>(&self, title: &'a str, pane: Focus) -> Block<'a> {
        let style = if self.focus == pane {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(style)
    }
}
