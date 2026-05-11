use crate::Source;

mod claude_code;
mod claude_desktop;
mod cline;
mod cursor;
mod lm_studio;
mod opencode;
mod windsurf;
mod zed;

pub use claude_code::ClaudeCode;
pub use claude_desktop::ClaudeDesktop;
pub use cline::Cline;
pub use cursor::Cursor;
pub use lm_studio::LmStudio;
pub use opencode::Opencode;
pub use windsurf::Windsurf;
pub use zed::Zed;

pub fn registry() -> Vec<Box<dyn Source>> {
    vec![
        Box::new(ClaudeCode),
        Box::new(ClaudeDesktop),
        Box::new(Cursor),
        Box::new(LmStudio),
        Box::new(Windsurf),
        Box::new(Cline),
        Box::new(Zed),
        Box::new(Opencode),
    ]
}
