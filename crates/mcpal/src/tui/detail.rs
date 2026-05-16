use crossterm::event::{KeyCode, KeyEvent};
use mcpal_core::Client;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};
use mcpal_core::rmcp::model::{Prompt, Resource, Tool};

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
    fn next(self) -> Self {
        match self {
            Tab::Tools => Tab::Resources,
            Tab::Resources => Tab::Prompts,
            Tab::Prompts => Tab::Tools,
        }
    }
}

pub struct Loaded {
    pub reference: String,
    pub tools: Vec<Tool>,
    pub resources: Vec<Resource>,
    pub prompts: Vec<Prompt>,
}

pub enum View {
    Empty,
    Connecting(String),
    Failed { reference: String, err: String },
    Server {
        loaded: Loaded,
        tab: Tab,
        state: ListState,
    },
    Schema(Tool),
}

impl View {
    pub fn server(loaded: Loaded) -> Self {
        let mut state = ListState::default();
        if !loaded.tools.is_empty() {
            state.select(Some(0));
        }
        View::Server {
            loaded,
            tab: Tab::Tools,
            state,
        }
    }
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
                    format!("{reference}: failed"),
                    Style::default().fg(Color::Red),
                )),
                Line::from(err.as_str()),
                Line::from(""),
                Line::from("r retry  Esc back"),
            ]);
            f.render_widget(p, inner);
        }
        View::Server { loaded, tab, state } => render_server(loaded, *tab, state, f, inner),
        View::Schema(tool) => render_schema(tool, f, inner),
    }
}

fn render_server(loaded: &Loaded, tab: Tab, state: &mut ListState, f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    let counts = [
        loaded.tools.len(),
        loaded.resources.len(),
        loaded.prompts.len(),
    ];
    let titles: Vec<Line> = Tab::titles()
        .iter()
        .zip(counts)
        .map(|(t, n)| Line::from(format!("{t} ({n})")))
        .collect();
    let tabs = Tabs::new(titles)
        .select(tab.index())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(tabs, chunks[0]);

    let items: Vec<ListItem> = match tab {
        Tab::Tools => loaded
            .tools
            .iter()
            .map(|t| {
                let desc = t.description.as_deref().unwrap_or("");
                let line = if desc.is_empty() {
                    t.name.to_string()
                } else {
                    format!("{}  {}", t.name, desc)
                };
                ListItem::new(line)
            })
            .collect(),
        Tab::Resources => loaded
            .resources
            .iter()
            .map(|r| {
                let name = r.raw.name.as_str();
                let uri = r.raw.uri.as_str();
                ListItem::new(format!("{name}  {uri}"))
            })
            .collect(),
        Tab::Prompts => loaded
            .prompts
            .iter()
            .map(|p| {
                let d = p.description.as_deref().unwrap_or("");
                let line = if d.is_empty() {
                    p.name.clone()
                } else {
                    format!("{}  {}", p.name, d)
                };
                ListItem::new(line)
            })
            .collect(),
    };
    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, chunks[1], state);
}

fn render_schema(tool: &Tool, f: &mut Frame, area: Rect) {
    let schema = serde_json::to_string_pretty(&*tool.input_schema)
        .unwrap_or_else(|e| format!("<schema error: {e}>"));
    let mut lines = vec![Line::from(Span::styled(
        tool.name.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(desc) = tool.description.as_deref() {
        lines.push(Line::from(desc.to_string()));
    }
    lines.push(Line::from(""));
    lines.extend(schema.lines().map(|l| Line::from(l.to_string())));
    lines.push(Line::from(""));
    lines.push(Line::from("Esc back"));
    f.render_widget(Paragraph::new(lines), area);
}

pub fn on_key(view: &mut View, key: KeyEvent) -> bool {
    match view {
        View::Server { loaded, tab, state } => on_server_key(loaded, tab, state, key),
        View::Schema(_) => {
            if matches!(key.code, KeyCode::Esc) {
                // Caller (App) downgrades to last Server view; signal handled.
                return true;
            }
            false
        }
        _ => false,
    }
}

fn on_server_key(
    loaded: &Loaded,
    tab: &mut Tab,
    state: &mut ListState,
    key: KeyEvent,
) -> bool {
    if matches!(key.code, KeyCode::Right | KeyCode::Char('l')) {
        *tab = tab.next();
        reset_state(state, len_for(loaded, *tab));
        return true;
    }
    let len = len_for(loaded, *tab);
    if len == 0 {
        return false;
    }
    let cur = state.selected().unwrap_or(0);
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if cur + 1 < len => {
            state.select(Some(cur + 1));
            true
        }
        KeyCode::Char('k') | KeyCode::Up if cur > 0 => {
            state.select(Some(cur - 1));
            true
        }
        KeyCode::Char('g') => {
            state.select(Some(0));
            true
        }
        KeyCode::Char('G') => {
            state.select(Some(len - 1));
            true
        }
        _ => false,
    }
}

fn len_for(loaded: &Loaded, tab: Tab) -> usize {
    match tab {
        Tab::Tools => loaded.tools.len(),
        Tab::Resources => loaded.resources.len(),
        Tab::Prompts => loaded.prompts.len(),
    }
}

fn reset_state(state: &mut ListState, len: usize) {
    state.select(if len == 0 { None } else { Some(0) });
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
