//! Approval modal overlay.

use crate::app::PendingApproval;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render(f: &mut ratatui::Frame, pending: &PendingApproval, screen: Rect) {
    let popup = centered_rect(62, 12, screen);
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
        Constraint::Min(3),    // tool + preview
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

    let body = vec![
        Line::from(format!("tool: {}", pending.tool)),
        Line::from(""),
        Line::from(wrap_preview(&pending.input_preview, rows[1].width as usize)),
    ];
    f.render_widget(Paragraph::new(body), rows[1]);

    if let Some(rule) = &pending.suggested_rule {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("prefix: {rule}"),
                Style::default().fg(Color::DarkGray),
            ))),
            rows[2],
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
    f.render_widget(Paragraph::new(legend), rows[3]);
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
    let w = (r.width.saturating_mul(percent_x) / 100).clamp(24, r.width);
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
