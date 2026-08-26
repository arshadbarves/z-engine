//! Approval modal overlay — shows a unified-diff preview (syntax
//! highlighted) for editing tools, falling back to raw input JSON.

use crate::app::PendingApproval;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render(f: &mut ratatui::Frame, pending: &PendingApproval, screen: Rect) {
    let popup = centered_rect(78, 18, screen);
    f.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(outer, popup);

    let inner = popup.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let rows = Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Length(1), // tool line
        Constraint::Min(4),    // preview (diff or json)
        Constraint::Length(1), // suggested rule
        Constraint::Length(1), // legend
    ])
    .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "⚠ approval required",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(format!("tool: {}", pending.tool))),
        rows[1],
    );

    // Preview area: diff with syntax highlighting when available.
    let width = rows[2].width.max(8) as usize;
    let height = rows[2].height as usize;
    let lines = match &pending.detail_preview {
        Some(diff) => diff_lines(diff, width),
        None => vec![Line::from(wrap_preview(&pending.input_preview, width))],
    };
    let visible: Vec<Line> = lines.into_iter().take(height).collect();
    f.render_widget(Paragraph::new(visible), rows[2]);

    if let Some(rule) = &pending.suggested_rule {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("prefix: {rule}"),
                Style::default().fg(Color::DarkGray),
            ))),
            rows[3],
        );
    }

    let bold_green = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);
    let legend = Line::from(vec![
        Span::styled("y", bold_green),
        Span::raw(" once · "),
        Span::styled("a", bold_green),
        Span::raw(" always prefix · "),
        Span::styled(
            "n",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("/Esc/Ctrl-C deny"),
    ]);
    f.render_widget(Paragraph::new(legend), rows[4]);
}

/// Build styled lines from a unified diff: hunk headers cyan, `+` green,
/// `-` red; code content syntax-highlighted by the target file extension.
fn diff_lines(diff: &str, width: usize) -> Vec<Line<'static>> {
    let ext = diff
        .lines()
        .find_map(|l| l.strip_prefix("+++ b/"))
        .and_then(|p| p.rsplit('.').next())
        .map(str::to_string);

    let mut out: Vec<Line<'static>> = Vec::new();
    let mut hl_state = None; // syntect state persists across the diff
    for raw in diff.lines() {
        for chunk in wrap_str(raw, width) {
            let mut spans: Vec<Span<'static>> = Vec::new();

            let first = chunk.chars().next();
            if let Some(marker) = first {
                let style = match marker {
                    '+' => Some(Style::default().fg(Color::Green)),
                    '-' => Some(Style::default().fg(Color::Red)),
                    '@' => Some(Style::default().fg(Color::Cyan)),
                    _ => None,
                };
                if let Some(style) = style {
                    spans.push(Span::styled(
                        marker.to_string(),
                        style.add_modifier(Modifier::BOLD),
                    ));
                }
            }

            let is_header =
                chunk.starts_with("---") || chunk.starts_with("+++") || chunk.starts_with("diff ");
            let marker_len = first.map(|c| c.len_utf8()).unwrap_or(0);
            let body_text = &chunk[marker_len..];

            if !is_header && matches!(first, Some('+') | Some('-')) {
                spans.extend(crate::views::syntax::spans_for_code(
                    body_text,
                    ext.as_deref(),
                    &mut hl_state,
                ));
            } else if marker_len > 0 {
                spans.push(Span::raw(body_text.to_string()));
            }
            out.push(Line::from(spans));
        }
    }
    out
}

fn wrap_str(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for src in text.split('\n') {
        let mut cur = String::new();
        for c in src.chars() {
            cur.push(c);
            if cur.chars().count() >= width {
                out.push(std::mem::take(&mut cur));
            }
        }
        out.push(cur);
    }
    out
}

fn wrap_preview(text: &str, width: usize) -> String {
    let width = width.max(8);
    let mut out = String::new();
    let mut count = 0usize;
    for c in text.chars() {
        if count >= width {
            out.push('\n');
            count = 0;
        }
        out.push(c);
        count += 1;
    }
    out
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    // `clamp(min, max)` panics when min > max — i.e. any terminal
    // narrower than 24 columns would crash the whole app mid-draw.
    let w = (r.width.saturating_mul(percent_x) / 100)
        .min(r.width)
        .max(24.min(r.width));
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let h = height.min(r.height.saturating_sub(2));
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
