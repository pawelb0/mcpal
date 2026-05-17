use mcpal_core::Client;
use mcpal_core::rmcp::model::CallToolResult;
use mcpal_core::rmcp::model::{Prompt, Resource, Tool};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tab {
    Tools,
    Resources,
    Prompts,
}

impl Tab {
    fn titles() -> [&'static str; 3] {
        ["Tools", "Resources", "Prompts"]
    }
    fn index(self) -> usize {
        match self {
            Tab::Tools => 0,
            Tab::Resources => 1,
            Tab::Prompts => 2,
        }
    }
    pub fn next(self) -> Self {
        match self {
            Tab::Tools => Tab::Resources,
            Tab::Resources => Tab::Prompts,
            Tab::Prompts => Tab::Tools,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Tab::Tools => Tab::Prompts,
            Tab::Resources => Tab::Tools,
            Tab::Prompts => Tab::Resources,
        }
    }
}

pub struct Loaded {
    pub reference: String,
    pub tools: Vec<Tool>,
    pub resources: Vec<Resource>,
    pub prompts: Vec<Prompt>,
}

pub struct ServerView {
    pub loaded: Loaded,
    pub tab: Tab,
    pub state: ListState,
}

impl ServerView {
    pub fn new(loaded: Loaded) -> Self {
        let mut state = ListState::default();
        if !loaded.tools.is_empty() {
            state.select(Some(0));
        }
        Self {
            loaded,
            tab: Tab::Tools,
            state,
        }
    }

    pub fn len(&self) -> usize {
        match self.tab {
            Tab::Tools => self.loaded.tools.len(),
            Tab::Resources => self.loaded.resources.len(),
            Tab::Prompts => self.loaded.prompts.len(),
        }
    }

    pub fn reset_selection(&mut self) {
        let len = self.len();
        self.state.select(if len == 0 { None } else { Some(0) });
    }

    pub fn move_by(&mut self, delta: isize) {
        let len = self.len();
        if len == 0 {
            return;
        }
        let cur = self.state.selected().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, len as isize - 1) as usize;
        self.state.select(Some(next));
    }

    pub fn top(&mut self) {
        if self.len() > 0 {
            self.state.select(Some(0));
        }
    }

    pub fn bottom(&mut self) {
        let len = self.len();
        if len > 0 {
            self.state.select(Some(len - 1));
        }
    }

    pub fn selected_tool(&self) -> Option<&Tool> {
        if self.tab != Tab::Tools {
            return None;
        }
        self.loaded.tools.get(self.state.selected()?)
    }
}

pub enum View {
    Empty,
    Connecting(String),
    Failed {
        reference: String,
        err: String,
    },
    Server(ServerView),
    Schema {
        parent: ServerView,
        tool: Tool,
    },
    CallResult {
        parent: ServerView,
        tool: String,
        outcome: Result<CallToolResult, String>,
    },
}

pub fn render(view: &mut View, f: &mut Frame, area: Rect, focused: bool) {
    let border = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title("Detail");
    let inner = block.inner(area);
    f.render_widget(block, area);
    match view {
        View::Empty => {
            f.render_widget(Paragraph::new("Enter to open a server."), inner);
        }
        View::Connecting(r) => {
            f.render_widget(Paragraph::new(format!("connecting to {r}…")), inner);
        }
        View::Failed { reference, err } => {
            let p = Paragraph::new(vec![
                Line::from(Span::styled(
                    sanitize(&format!("{reference}: failed")),
                    Style::default().fg(Color::Red),
                )),
                Line::from(sanitize(err)),
                Line::from(""),
                Line::from("Esc back · b set bearer"),
            ]);
            f.render_widget(p, inner);
        }
        View::Server(sv) => render_server(sv, f, inner),
        View::Schema { tool, .. } => render_schema(tool, f, inner),
        View::CallResult { tool, outcome, .. } => render_call_result(tool, outcome, f, inner),
    }
}

fn render_call_result(
    tool: &str,
    outcome: &Result<CallToolResult, String>,
    f: &mut Frame,
    area: Rect,
) {
    let (header, body, ui_badge) = match outcome {
        Ok(r) => {
            let mark = if r.is_error.unwrap_or(false) {
                Span::styled("✗", Style::default().fg(Color::Red))
            } else {
                Span::styled("✓", Style::default().fg(Color::Green))
            };
            let body = serde_json::to_string_pretty(r).unwrap_or_else(|e| e.to_string());
            let badge = if crate::commands::ui::has_ui(r) {
                Some(Span::styled(
                    "  UI ✦",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                None
            };
            (mark, body, badge)
        }
        Err(e) => (
            Span::styled("✗", Style::default().fg(Color::Red)),
            e.clone(),
            None,
        ),
    };
    let mut header_spans = vec![
        header,
        Span::raw(" "),
        Span::styled(
            sanitize(tool),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(b) = ui_badge {
        header_spans.push(b);
    }
    let mut lines = vec![Line::from(header_spans)];
    lines.push(Line::from(""));
    lines.extend(body.lines().map(|l| Line::from(sanitize(l))));
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Esc back · c call again · :ui inspect to save the UI payload",
    ));
    f.render_widget(Paragraph::new(lines), area);
}

pub fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\n' || c == '\t' {
                c
            } else if c.is_control() {
                '·'
            } else {
                c
            }
        })
        .collect()
}

fn render_server(sv: &mut ServerView, f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let counts = [
        sv.loaded.tools.len(),
        sv.loaded.resources.len(),
        sv.loaded.prompts.len(),
    ];
    let titles: Vec<Line> = Tab::titles()
        .iter()
        .zip(counts)
        .map(|(t, n)| Line::from(format!("{t} ({n})")))
        .collect();
    let tabs = Tabs::new(titles)
        .select(sv.tab.index())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(tabs, chunks[0]);

    let items: Vec<ListItem> = match sv.tab {
        Tab::Tools => sv
            .loaded
            .tools
            .iter()
            .map(|t| {
                let desc = t.description.as_deref().unwrap_or("");
                let line = if desc.is_empty() {
                    sanitize(&t.name)
                } else {
                    sanitize(&format!("{}  {}", t.name, desc))
                };
                ListItem::new(line)
            })
            .collect(),
        Tab::Resources => sv
            .loaded
            .resources
            .iter()
            .map(|r| ListItem::new(sanitize(&format!("{}  {}", r.raw.name, r.raw.uri))))
            .collect(),
        Tab::Prompts => sv
            .loaded
            .prompts
            .iter()
            .map(|p| {
                let d = p.description.as_deref().unwrap_or("");
                let line = if d.is_empty() {
                    sanitize(&p.name)
                } else {
                    sanitize(&format!("{}  {}", p.name, d))
                };
                ListItem::new(line)
            })
            .collect(),
    };
    let list = List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, chunks[1], &mut sv.state);
}

fn render_schema(tool: &Tool, f: &mut Frame, area: Rect) {
    let schema = serde_json::to_string_pretty(&*tool.input_schema)
        .unwrap_or_else(|e| format!("<schema error: {e}>"));
    let mut lines = vec![Line::from(Span::styled(
        sanitize(&tool.name),
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(desc) = tool.description.as_deref() {
        lines.push(Line::from(sanitize(desc)));
    }
    lines.push(Line::from(""));
    lines.extend(schema.lines().map(|l| Line::from(sanitize(l))));
    lines.push(Line::from(""));
    lines.push(Line::from("Esc back · c call"));
    f.render_widget(Paragraph::new(lines), area);
}

pub async fn open(
    reference: String,
    mut spec: mcpal_core::ServerSpec,
    handler: mcpal_core::Handler,
) -> anyhow::Result<(Client, Loaded)> {
    crate::runtime::attach_bearer(&mut spec, &reference, &reference).await;
    let client = mcpal_core::connect(&spec, handler).await?;
    let tools = client.list_all_tools().await.unwrap_or_default();
    let resources = client.list_all_resources().await.unwrap_or_default();
    let prompts = client.list_all_prompts().await.unwrap_or_default();
    Ok((
        client,
        Loaded {
            reference,
            tools,
            resources,
            prompts,
        },
    ))
}
