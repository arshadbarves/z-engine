//! Status bar: model · tokens · est. cost · session id.
//!
//! Cost shows "–" until per-model pricing lands (v1.0 calibration); token
//! meter turns amber ≥80% and red ≥92% of budget (auto-compaction arrives
//! with v0.3).

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn render(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let total_tokens = app.prompt_tokens + app.completion_tokens;
    let budget = u64::from(app.max_context_tokens);
    let ratio = total_tokens as f32 / budget as f32;

    let token_style = if ratio >= 0.92 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if ratio >= 0.80 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let status_word = if app.turn_active {
        Span::styled(" ● working", Style::default().fg(Color::Green))
    } else if app.pending.is_some() {
        Span::styled(
            " ◆ approval",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" ○ idle", Style::default().fg(Color::DarkGray))
    };

    let line = Line::from(vec![
        status_word,
        Span::raw(" · "),
        Span::styled(
            app.model.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{total_tokens} tok ({:.0}%)", ratio * 100.0),
            token_style,
        ),
        Span::raw(" · $–"),
        Span::raw(" · session "),
        Span::styled(app.session_tag.clone(), Style::default().fg(Color::Cyan)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
