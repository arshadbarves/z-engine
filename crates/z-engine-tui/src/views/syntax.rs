//! Syntax-highlighting bridge: syntect styles → ratatui spans.

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

struct Engine {
    ss: SyntaxSet,
    theme: Theme,
}

fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(|| {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = ts
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| ts.themes.values().next().cloned().unwrap_or_default());
        Engine { ss, theme }
    })
}

fn syntax_for(ext: Option<&str>) -> Option<&'static SyntaxReference> {
    let e = engine();
    match ext {
        Some(ext) => {
            e.ss.find_syntax_by_extension(ext)
                .or_else(|| e.ss.find_syntax_by_token(ext))
        }
        None => None,
    }
    .or_else(|| Some(e.ss.find_syntax_plain_text()))
}

fn to_ratatui_color(c: syntect::highlighting::Color) -> Option<Color> {
    if c.a == 0 {
        None
    } else {
        Some(Color::Rgb(c.r, c.g, c.b))
    }
}

/// Highlight one line of code in the given language extension, returning
/// styled spans suitable for a ratatui `Line`.
pub fn spans_for_code(
    line: &str,
    ext: Option<&str>,
    state: &mut Option<HighlightLines>,
) -> Vec<Span<'static>> {
    let e = engine();
    let syn = syntax_for(ext).expect("plain text fallback exists");
    let hl = state.get_or_insert_with(|| HighlightLines::new(syn, &e.theme));
    let Ok(ranges) = hl.highlight_line(line, &e.ss) else {
        return vec![Span::raw(line.to_string())];
    };
    ranges
        .into_iter()
        .filter(|(_, text)| !text.is_empty())
        .map(|(style, text)| {
            let mut st = Style::default();
            if let Some(fg) = to_ratatui_color(style.foreground) {
                st = st.fg(fg);
            }
            use syntect::highlighting::FontStyle;
            if style.font_style.contains(FontStyle::BOLD) {
                st = st.add_modifier(Modifier::BOLD);
            }
            if style.font_style.contains(FontStyle::UNDERLINE) {
                st = st.add_modifier(Modifier::UNDERLINED);
            }
            Span::styled(text.trim_end_matches('\n').to_string(), st)
        })
        .collect()
}
