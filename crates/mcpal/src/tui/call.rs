use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};
use mcpal_core::rmcp::model::Tool;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    String,
    Integer,
    Number,
    Boolean,
    Json,
}

impl FieldKind {
    fn from_str(s: &str) -> Self {
        match s {
            "integer" => Self::Integer,
            "number" => Self::Number,
            "boolean" => Self::Boolean,
            "object" | "array" => Self::Json,
            _ => Self::String,
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::String => "str",
            Self::Integer => "int",
            Self::Number => "num",
            Self::Boolean => "bool",
            Self::Json => "json",
        }
    }
}

pub struct Field {
    pub name: String,
    pub required: bool,
    pub kind: FieldKind,
    pub buf: String,
}

pub struct CallForm {
    pub tool: Tool,
    pub fields: Vec<Field>,
    pub current: usize,
}

impl CallForm {
    pub fn new(tool: Tool) -> Self {
        let fields = fields_from_schema(&tool.input_schema);
        Self {
            tool,
            fields,
            current: 0,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Esc => Outcome::Cancel,
            KeyCode::Enter => Outcome::Submit,
            KeyCode::Tab => {
                if !self.fields.is_empty() {
                    self.current = (self.current + 1) % self.fields.len();
                }
                Outcome::Handled
            }
            KeyCode::BackTab => {
                if !self.fields.is_empty() {
                    self.current = (self.current + self.fields.len() - 1) % self.fields.len();
                }
                Outcome::Handled
            }
            KeyCode::Backspace => {
                if let Some(f) = self.fields.get_mut(self.current) {
                    f.buf.pop();
                }
                Outcome::Handled
            }
            KeyCode::Char(c) => {
                if let Some(f) = self.fields.get_mut(self.current) {
                    f.buf.push(c);
                }
                Outcome::Handled
            }
            _ => Outcome::Handled,
        }
    }

    pub fn to_json(&self) -> Result<Value, String> {
        let mut obj = Map::new();
        for f in &self.fields {
            if f.buf.is_empty() && !f.required {
                continue;
            }
            let parsed = parse_field(&f.buf, f.kind)
                .map_err(|e| format!("{}: {e}", f.name))?;
            obj.insert(f.name.clone(), parsed);
        }
        Ok(Value::Object(obj))
    }
}

pub enum Outcome {
    Handled,
    Submit,
    Cancel,
}

pub fn render(form: &CallForm, f: &mut Frame, area: Rect) {
    let area = popup_area(area, 60, 70);
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!("call {}", form.tool.name));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let lines: Vec<Line> = form
        .fields
        .iter()
        .enumerate()
        .flat_map(|(i, fld)| {
            let active = i == form.current;
            let header_style = if active {
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)
            } else {
                Style::default()
            };
            let req = if fld.required { "*" } else { " " };
            let header = Line::from(vec![
                Span::styled(format!("{req} {}", fld.name), header_style),
                Span::raw(format!("  [{}]", fld.kind.label())),
            ]);
            let value = if active {
                format!(" › {}▌", fld.buf)
            } else {
                format!("   {}", fld.buf)
            };
            vec![header, Line::from(value), Line::from("")]
        })
        .collect();
    f.render_widget(Paragraph::new(lines), rows[0]);
    f.render_widget(
        Paragraph::new("Enter submit  Esc cancel  Tab next field").style(
            Style::default().fg(Color::DarkGray),
        ),
        rows[1],
    );
}

fn fields_from_schema(schema: &serde_json::Map<String, Value>) -> Vec<Field> {
    let props = match schema.get("properties").and_then(|v| v.as_object()) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let required: HashSet<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    props
        .iter()
        .map(|(name, def)| {
            let ty = def.get("type").and_then(|v| v.as_str()).unwrap_or("string");
            let kind = FieldKind::from_str(ty);
            Field {
                name: name.clone(),
                required: required.contains(name),
                kind,
                buf: prefill(def, kind),
            }
        })
        .collect()
}

fn prefill(def: &Value, kind: FieldKind) -> String {
    // Explicit default wins.
    if let Some(d) = def.get("default") {
        return scalar(d);
    }
    // Enum's first option is a sensible starting point.
    if let Some(first) = def.get("enum").and_then(Value::as_array).and_then(|a| a.first()) {
        return scalar(first);
    }
    match kind {
        // Single-line compact JSON keeps the modal readable; submit accepts
        // any valid JSON.
        FieldKind::Json => serde_json::to_string(&example(def)).unwrap_or_default(),
        _ => String::new(),
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn example(s: &Value) -> Value {
    match s.get("type").and_then(Value::as_str).unwrap_or("") {
        "object" => Value::Object(
            s.get("properties")
                .and_then(Value::as_object)
                .map(|p| p.iter().map(|(k, v)| (k.clone(), example(v))).collect())
                .unwrap_or_default(),
        ),
        "array" => Value::Array(vec![
            s.get("items").map(example).unwrap_or(Value::Null),
        ]),
        "string" => Value::String(String::new()),
        "integer" | "number" => Value::Number(0.into()),
        "boolean" => Value::Bool(false),
        _ => Value::Null,
    }
}

fn parse_field(buf: &str, kind: FieldKind) -> Result<Value, String> {
    if buf.is_empty() {
        return Err("required field is empty".into());
    }
    Ok(match kind {
        FieldKind::String => Value::String(buf.into()),
        FieldKind::Integer => buf
            .parse::<i64>()
            .map(|n| Value::Number(n.into()))
            .map_err(|e| e.to_string())?,
        FieldKind::Number => serde_json::from_str(buf).map_err(|e| e.to_string())?,
        FieldKind::Boolean => match buf {
            "true" | "yes" | "1" => Value::Bool(true),
            "false" | "no" | "0" => Value::Bool(false),
            _ => return Err("expected true/false".into()),
        },
        FieldKind::Json => serde_json::from_str(buf).map_err(|e| e.to_string())?,
    })
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
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
