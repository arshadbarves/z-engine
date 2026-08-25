//! Inline terminal output (v1.1): append-only styled printing.
//!
//! The transcript lives in the terminal's own scrollback — finished lines
//! are printed once and never touched again. Only the *live* pieces (the
//! streaming assistant tail and the input/status rows) are rewritten in
//! place, tracked by row counts so soft-wrapped lines behave correctly.
//!
//! No alternate screen, no mouse capture: selection, copy, and the
//! terminal's native scrolling work like any regular CLI program.

use std::io::Write;

/// A styled span ready to be emitted as ANSI.
#[derive(Debug, Clone)]
pub struct AnsiSpan {
    pub text: String,
    pub fg: Option<Rgb>,
    pub bold: bool,
    pub dim: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const CYAN: Rgb = Rgb(0x00, 0xB7, 0xC7);
    pub const GREEN: Rgb = Rgb(0x00, 0xBB, 0x55);
    pub const RED: Rgb = Rgb(0xFF, 0x55, 0x55);
    pub const YELLOW: Rgb = Rgb(0xE5, 0xC0, 0x7F);
    pub const GRAY: Rgb = Rgb(0x88, 0x88, 0x88);
    #[allow(dead_code)]
    pub const MAGENTA: Rgb = Rgb(0xC7, 0x87, 0xD7);
    #[allow(dead_code)]
    pub const WHITE: Rgb = Rgb(0xEE, 0xEE, 0xEE);
}

fn fg_code(c: Rgb) -> String {
    format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2)
}

fn spans_to_ansi(spans: &[AnsiSpan]) -> String {
    let mut out = String::new();
    for s in spans {
        let mut codes = String::new();
        if let Some(fg) = s.fg {
            codes.push_str(&fg_code(fg));
        }
        if s.bold {
            codes.push_str("\x1b[1m");
        }
        if s.dim {
            codes.push_str("\x1b[2m");
        }
        let reset = if codes.is_empty() { "" } else { "\x1b[0m" };
        out.push_str(&format!("{codes}{}{reset}", s.text));
    }
    out
}

/// Printer owning stdout. All output funnels through here so raw-mode
/// bookkeeping stays in one place.
pub struct Printer {
    width: usize,
    /// Assistant text received but not yet flushed as complete lines.
    stream_buf: String,
}

impl Printer {
    pub fn new(width: usize) -> Self {
        Self {
            width: width.max(20),
            stream_buf: String::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    #[allow(dead_code)]
    pub fn set_width(&mut self, w: usize) {
        self.width = w.max(20);
    }

    /// Print a styled line into scrollback.
    pub fn println_spans(&mut self, spans: &[AnsiSpan]) {
        let line = spans_to_ansi(spans);
        self.emit_wrapped(&line);
    }

    /// Print a plain line into scrollback.
    pub fn println_plain(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.emit_wrapped(&text);
    }

    /// Feed streaming assistant tokens; completed lines print immediately,
    /// the trailing partial stays buffered until `end_stream`.
    pub fn push_stream(&mut self, delta: String) {
        self.stream_buf.push_str(&delta);
        while let Some(i) = self.stream_buf.find('\n') {
            let line: String = self.stream_buf.drain(..=i).collect();
            let line = line.trim_end_matches('\n');
            self.emit_wrapped(line);
        }
    }

    /// Flush any trailing partial line and close the streamed block.
    pub fn end_stream(&mut self) {
        if !self.stream_buf.is_empty() {
            let rest = std::mem::take(&mut self.stream_buf);
            self.emit_wrapped(&rest);
        }
    }

    /// Rewrite the two bottom rows (status pill above prompt) in place.
    pub fn rewrite_bottom_two(&mut self, status: &str, prompt: &str) {
        // We are on row 2 of 2 → up one, clear, status; down, clear, prompt.
        print!("\x1b[1A\r\x1b[2K{status}\r\n\x1b[2K{prompt}");
        let _ = std::io::stdout().flush();
    }

    /// Emit text honoring the width (soft-wrap). Returns rows written.
    fn emit_wrapped(&mut self, text: &str) -> usize {
        let mut rows = 0usize;
        for src_line in wrap_text(text, self.width) {
            println!("{src_line}");
            rows += 1;
        }
        let _ = std::io::stdout().flush();
        rows
    }
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested without any terminal)
// ---------------------------------------------------------------------------

/// Split `text` into display rows of at most `width` chars.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(4);
    let mut out = Vec::new();
    for src in text.split('\n') {
        let mut cur = String::new();
        for ch in src.chars() {
            cur.push(ch);
            if cur.chars().count() >= width {
                out.push(std::mem::take(&mut cur));
            }
        }
        // Preserve blank lines; avoid a phantom empty row after a flush.
        if !cur.is_empty() || src.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// How many physical rows `text` occupies at `width`.
#[cfg(test)]
pub fn row_count(text: &str, width: usize) -> usize {
    wrap_text(text, width).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_respects_width_and_newlines() {
        assert_eq!(wrap_text("abc", 10), vec!["abc"]);
        assert_eq!(wrap_text("abcdef", 4), vec!["abcd", "ef"]);
        assert_eq!(wrap_text("ab\ncd", 10), vec!["ab", "cd"]);
        assert_eq!(wrap_text("", 10), vec![""]);
    }

    #[test]
    fn row_count_counts_each_logical_line() {
        assert_eq!(row_count("one\ntwo\nthree", 40), 3);
        assert_eq!(row_count("a".repeat(80).as_str(), 40), 2);
    }

    #[test]
    fn ansi_spans_build_codes() {
        let s = spans_to_ansi(&[AnsiSpan {
            text: "ok".into(),
            fg: Some(Rgb::GREEN),
            bold: true,
            dim: false,
        }]);
        assert!(s.contains("\x1b[38;2;0;187;85m"));
        assert!(s.contains("\x1b[1m"));
        assert!(s.ends_with("ok\x1b[0m"));
    }
}
