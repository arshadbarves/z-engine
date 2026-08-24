//! Chat transcript rendering: blocks → styled, wrapped lines, viewport
//! pinned to the bottom unless the user scrolled up.

use crate::app::{App, Block};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

pub fn render(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let width = area.width.saturating_sub(2).max(1) as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();

    for block in &app.blocks {
        match block {
            Block::User(text) => {
                wrap_into(
                    &mut lines,
                    format!("you ❯ {text}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    width,
                );
                lines.push(Line::from(""));
            }
            Block::Assistant { text, streaming } => {
                let mut t = text.clone();
                if *streaming {
                    t.push('▌');
                }
                wrap_into(&mut lines, t, Style::default(), width);
                if !*streaming {
                    lines.push(Line::from(""));
                }
            }
            Block::ToolCall {
                name,
                preview,
                summary,
                ok,
                done,
            } => {
                let (glyph, style) = match (*done, *ok) {
                    (false, _) => ("⚙", Style::default().fg(Color::Yellow)),
                    (true, true) => ("✓", Style::default().fg(Color::Green)),
                    (true, false) => ("✗", Style::default().fg(Color::Red)),
                };
                let detail = if summary.is_empty() {
                    preview.clone()
                } else {
                    summary.clone()
                };
                wrap_into(
                    &mut lines,
                    format!("{glyph} {name} ─ {detail}"),
                    style.add_modifier(Modifier::DIM),
                    width,
                );
            }
            Block::Notice(s) => {
                wrap_into(
                    &mut lines,
                    s.clone(),
                    Style::default().fg(Color::DarkGray),
                    width,
                );
            }
            Block::Error(s) => {
                wrap_into(
                    &mut lines,
                    format!("ERROR: {s}"),
                    Style::default().fg(Color::Red),
                    width,
                );
                lines.push(Line::from(""));
            }
        }
    }

    // Viewport: last `height` lines above `scroll_from_bottom`.
    let height = area.height as usize;
    let end = lines.len().saturating_sub(app.scroll_from_bottom as usize);
    let start = end.saturating_sub(height);
    let visible: Vec<Line<'static>> = lines[start..end].to_vec();

    f.render_widget(Paragraph::new(visible).wrap(Wrap { trim: false }), area);
}

/// Push `text` into `lines`, hard-wrapping each source line to `width`
/// chars so bottom-pinning math stays trivial.
fn wrap_into(lines: &mut Vec<Line<'static>>, text: String, style: Style, width: usize) {
    for src_line in text.split('\n') {
        let mut chunk = String::new();
        let flush = |chunk: &mut String, lines: &mut Vec<Line<'static>>| {
            let owned = chunk.clone();
            chunk.clear();
            lines.push(Line::from(Span::styled(owned, style)));
        };
        for c in src_line.chars() {
            chunk.push(c);
            if chunk.chars().count() >= width {
                flush(&mut chunk, lines);
            }
        }
        // trailing partial (or empty) line still renders
        flush(&mut chunk, lines);
    }
}
