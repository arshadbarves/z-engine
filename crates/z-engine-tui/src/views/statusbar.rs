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
use z_engine_core::context::cost;

pub fn render(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let total_tokens = app.prompt_tokens + app.completion_tokens;
    // Guard a hand-edited zero budget: division would yield NaN and
    // every threshold below silently compares false.
    let budget = u64::from(app.max_context_tokens);
    let ratio = if budget == 0 {
        0.0
    } else {
        total_tokens as f32 / budget as f32
    };

    let token_style = if ratio >= 0.92 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if ratio >= 0.80 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let status_word = if app.turn_active {
        let secs = app
            .turn_started_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        Span::styled(
            format!(" ● working {secs}s"),
            Style::default().fg(Color::Green),
        )
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
            format!("[{}]", app.ui_mode.label()),
            Style::default().fg(Color::LightMagenta),
        ),
        Span::raw(" · "),
        Span::styled(app.project_name.clone(), Style::default().fg(Color::Cyan)),
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
        Span::raw(" · "),
        {
            let usd = cost::cost_usd(app.pricing, app.prompt_tokens, app.completion_tokens);
            match usd {
                Some(c) => Span::raw(format!("${c:.4}")),
                None => Span::raw("$–").style(Style::default().fg(Color::DarkGray)),
            }
        },
        Span::raw(" · session "),
        Span::styled(app.session_tag.clone(), Style::default().fg(Color::Cyan)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
