mod app;
mod call;
mod detail;
mod output;
mod sidebar;

use std::io;

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::runtime::Ctx;

pub async fn run(ctx: &Ctx) -> Result<()> {
    // Child stderr would corrupt the alt-screen. Pin null for our scope.
    // SAFETY: process-wide env mutation; called once at TUI entry.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("MCPAL_CHILD_STDERR", "null");
    }
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    app::App::new(ctx)?.run(&mut terminal).await
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = disable_raw_mode();
    }
}
