pub mod approval;
mod chat;
pub mod picker;
mod statusbar;
#[allow(dead_code)]
pub mod syntax;

use crate::app::App;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();
    let rows = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .split(area);

    chat::render(f, app, rows[0]);

    let title = if app.turn_active {
        " working… (Esc aborts) ".to_string()
    } else if let Some(p) = &app.pending {
        format!(" approval pending: {} ", p.tool)
    } else {
        " task (Enter=send · ↑↓ history) ".to_string()
    };
    let input = Paragraph::new(app.input.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(title),
        )
        .style(if app.pending.is_some() {
            Style::default().add_modifier(Modifier::DIM)
        } else {
            Style::default()
        });
    f.render_widget(input, rows[1]);

    if let Some(pending) = &app.pending {
        approval::render(f, pending, area);
    }
    statusbar::render(f, app, rows[2]);
}
