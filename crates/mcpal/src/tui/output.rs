use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct OutputBuffer {
    lines: VecDeque<Line<'static>>,
    cap: usize,
}

impl OutputBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            cap,
        }
    }

    pub fn push(&mut self, line: Line<'static>) {
        if self.lines.len() == self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn info<S: Into<String>>(&mut self, s: S) {
        self.push(Line::from(s.into()));
    }

    pub fn ok<S: Into<String>>(&mut self, s: S) {
        self.push(Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::raw(s.into()),
        ]));
    }

    pub fn err<S: Into<String>>(&mut self, s: S) {
        self.push(Line::from(vec![
            Span::styled("✗ ", Style::default().fg(Color::Red)),
            Span::raw(s.into()),
        ]));
    }

    pub fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let border = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title("Output");
        let inner = block.inner(area);
        f.render_widget(block, area);
        let height = inner.height as usize;
        let start = self.lines.len().saturating_sub(height);
        let visible: Vec<Line<'static>> = self.lines.iter().skip(start).cloned().collect();
        f.render_widget(Paragraph::new(visible), inner);
    }
}
