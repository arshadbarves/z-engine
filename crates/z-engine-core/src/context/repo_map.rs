//! Repo map (spec §9 v0.6): tree-sitter symbol outlines per Rust file,
//! reference-ranked against the working set, rendered as a compact map
//! injected right after the L0 system prompt.
//!
//! Design notes:
//! - only `*.rs` files are outlined in v0.6 (the registry is trivially
//!   extensible per-language);
//! - ranking counts references of each symbol across the *tracked* working
//!   set first (files the model has read/edited), then the whole corpus;
//! - the renderer enforces a character budget so the map stays compact.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub kind: &'static str,
    pub name: String,
    pub line: usize,
}

#[derive(Debug, Default)]
pub struct FileOutline {
    pub symbols: Vec<Symbol>,
}

/// Node-kind → symbol label for declarations we outline.
fn symbol_kind(node_kind: &str) -> Option<&'static str> {
    Some(match node_kind {
        "function_item" => "fn",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "trait_item" => "trait",
        "mod_item" => "mod",
        "type_alias_declaration" => "type",
        "const_item" => "const",
        "static_item" => "static",
        _ => return None,
    })
}

fn walk<'a>(node: tree_sitter::Node<'a>, src: &'a [u8], out: &mut Vec<Symbol>) {
    if let Some(kind) = symbol_kind(node.kind()) {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(src) {
                out.push(Symbol {
                    kind,
                    name: name.to_string(),
                    line: node.start_position().row + 1,
                });
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, src, out);
    }
}

/// Extract an outline from Rust source. Returns None when nothing usable
/// parses.
pub fn extract_rust(source: &str) -> Option<FileOutline> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(source, None)?;
    let mut symbols = Vec::new();
    walk(tree.root_node(), source.as_bytes(), &mut symbols);
    if symbols.is_empty() {
        None
    } else {
        symbols.dedup();
        Some(FileOutline { symbols })
    }
}

/// Count occurrences of `needle` as a standalone-ish token inside text.
pub(crate) fn count_refs(text: &str, needle: &str) -> usize {
    if needle.len() < 3 {
        return 0; // avoid noise from tiny names
    }
    let mut count = 0usize;
    let mut rest = text;
    while let Some(pos) = rest.find(needle) {
        let before_ok =
            pos == 0 || !rest[..pos].ends_with(|c: char| c.is_alphanumeric() || c == '_');
        let after_idx = pos + needle.len();
        let after_ok = after_idx >= rest.len()
            || !rest[after_idx..].starts_with(|c: char| c.is_alphanumeric() || c == '_');
        if before_ok && after_ok {
            count += 1;
        }
        rest = &rest[after_idx..];
    }
    count
}

/// Ranked symbol entries ready for rendering (highest references first).
pub fn rank(
    outlines: &BTreeMap<PathBuf, Vec<Symbol>>,
    tracked: &BTreeSet<PathBuf>,
    corpus: &BTreeMap<PathBuf, String>,
) -> Vec<(PathBuf, Symbol, usize)> {
    let mut ranked: Vec<(PathBuf, Symbol, usize)> = Vec::new();
    for (path, symbols) in outlines {
        let in_working_set = tracked.contains(path);
        for sym in symbols {
            let mut refs = 0usize;
            for (cp, text) in corpus {
                if cp == path {
                    continue; // references counted across other files
                }
                refs += count_refs(text, &sym.name);
            }
            // Working-set boost: symbols in files the model touched matter.
            if in_working_set {
                refs += 1000;
            }
            ranked.push((path.clone(), sym.clone(), refs));
        }
    }
    ranked.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(a.0.cmp(&b.0))
            .then(a.1.line.cmp(&b.1.line))
    });
    ranked
}

/// Render the compact map within a character budget.
pub fn render(
    outlines: &BTreeMap<PathBuf, Vec<Symbol>>,
    tracked: &BTreeSet<PathBuf>,
    corpus: &BTreeMap<PathBuf, String>,
    budget_chars: usize,
) -> String {
    let ranked = rank(outlines, tracked, corpus);
    let mut out = String::from("# Repository symbol map (definition lines)\n");
    let mut included = 0usize;
    for (path, sym, _refs) in &ranked {
        let entry = format!(
            "{}:{} {} {}\n",
            path.display(),
            sym.line,
            sym.kind,
            sym.name
        );
        if out.chars().count() + entry.len() > budget_chars {
            break;
        }
        out.push_str(&entry);
        included += 1;
    }
    if included == 0 {
        return String::new();
    }
    out.push_str(&format!(
        "\n({included} symbols shown; use read_file on a definition to explore)"
    ));
    out
}

/// Walk the project and outline every Rust file (gitignore-aware).
/// Returns outlines plus each file's source for reference counting.
pub fn generate(
    project_root: &Path,
    max_files: usize,
    max_file_bytes: u64,
) -> (BTreeMap<PathBuf, Vec<Symbol>>, BTreeMap<PathBuf, String>) {
    let walker = ignore::WalkBuilder::new(project_root)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .build();

    let mut outlines = BTreeMap::new();
    let mut corpus = BTreeMap::new();
    for entry in walker.flatten() {
        if outlines.len() >= max_files {
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if entry
            .metadata()
            .map(|m| m.len() > max_file_bytes)
            .unwrap_or(true)
        {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(project_root) else {
            continue;
        };
        let Ok(src) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Some(outline) = extract_rust(&src) {
            corpus.insert(rel.to_path_buf(), src);
            outlines.insert(rel.to_path_buf(), outline.symbols);
        }
    }
    (outlines, corpus)
}

/// Regenerate the rendered map for a project from the current tracked set.
pub fn refresh_repo_map(ctx: &crate::tools::ToolCtx) -> String {
    const MAX_FILES: usize = 400;
    const MAX_FILE_BYTES: u64 = 200_000;
    const BUDGET_CHARS: usize = 6_000;

    let tracked = ctx.tracked_paths();
    let (outlines, corpus) = generate(&ctx.project_root, MAX_FILES, MAX_FILE_BYTES);
    render(&outlines, &tracked, &corpus, BUDGET_CHARS)
}

/// Find the identifier covering a 1-based line/column via tree-sitter.
pub fn identifier_at(path: &Path, line: usize, col: usize) -> Option<String> {
    let src = std::fs::read_to_string(path).ok()?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(&src, None)?;
    let root = tree.root_node();
    // 0-based offsets
    let row = line.saturating_sub(1);
    let column = col.saturating_sub(1);

    fn descend<'a>(
        node: tree_sitter::Node<'a>,
        row: usize,
        column: usize,
        src: &'a str,
    ) -> Option<String> {
        if !(node.start_position().row <= row && node.end_position().row >= row) {
            return None;
        }
        // Prefer the deepest named child containing the point.
        let mut best: Option<tree_sitter::Node> = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let sp = child.start_position();
            let ep = child.end_position();
            let contains = (sp.row < row || (sp.row == row && sp.column <= column))
                && (ep.row > row || (ep.row == row && ep.column >= column));
            if contains {
                match best {
                    Some(b)
                        if b.start_position() == child.start_position()
                            && b.end_position() == child.end_position() => {}
                    None => best = Some(child),
                    _ => {}
                }
                if let Some(found) = descend(child, row, column, src) {
                    return Some(found);
                }
                best = best.or(Some(child));
                let _ = &mut best;
            }
        }
        if node.kind() == "identifier" {
            let text = node.utf8_text(src.as_bytes()).ok()?.to_string();
            return Some(text);
        }
        best.and_then(|b| descend(b, row, column, src))
    }

    descend(root, row, column, &src)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"use std::collections::HashMap;

pub struct Zebra {
    pub stripes: u32,
}

pub enum Mood { Happy, Grumpy }

pub trait Runner {
    fn run(&self);
}

impl Runner for Zebra {
    fn run(&self) {}
}

const MAX_STRIPES: u32 = 99;

pub fn zebra_fn(z: &Zebra) -> u32 {
    z.stripes
}

mod hidden {}
"#;

    #[test]
    fn extracts_kinds_names_lines() {
        let o = extract_rust(SAMPLE).unwrap();
        let find = |k: &str, n: &str| o.symbols.iter().any(|s| s.kind == k && s.name == n);
        assert!(find("struct", "Zebra"));
        assert!(find("enum", "Mood"));
        assert!(find("trait", "Runner"));
        assert!(find("fn", "zebra_fn"));
        assert!(find("mod", "hidden"));
        assert!(find("const", "MAX_STRIPES"));
        // definition lines are 1-based and plausible
        let zf = o.symbols.iter().find(|s| s.name == "zebra_fn").unwrap();
        assert_eq!(zf.line, 19);
    }

    #[test]
    fn garbage_returns_none_not_panic() {
        assert!(extract_rust("\u{0}\u{1}\u{2}").is_none());
    }

    #[test]
    fn identifier_at_finds_symbol_under_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("s.rs");
        std::fs::write(&p, SAMPLE).unwrap();
        let id = identifier_at(&p, 19, 12); // somewhere inside "zebra_fn" name on its def line
        assert_eq!(id.as_deref(), Some("zebra_fn"));
    }

    #[test]
    fn ranking_prefers_referenced_symbols() {
        let mut outlines = BTreeMap::new();
        outlines.insert(
            PathBuf::from("src/lib.rs"),
            vec![
                Symbol {
                    kind: "fn",
                    name: "zebra_fn".into(),
                    line: 19,
                },
                Symbol {
                    kind: "fn",
                    name: "unused_thing".into(),
                    line: 40,
                },
            ],
        );
        let mut corpus = BTreeMap::new();
        corpus.insert(
            PathBuf::from("src/main.rs"),
            "fn main() { zebra_fn(); zebra_fn(); }".to_string(),
        );
        let ranked = rank(&outlines, &BTreeSet::new(), &corpus);
        assert_eq!(ranked[0].1.name, "zebra_fn");
        assert!(ranked[0].2 > ranked[1].2);
    }

    #[test]
    fn render_respects_budget() {
        let mut outlines = BTreeMap::new();
        outlines.insert(
            PathBuf::from("src/lib.rs"),
            (1..=50)
                .map(|i| Symbol {
                    kind: "fn",
                    name: format!("func_{i}"),
                    line: i,
                })
                .collect(),
        );
        let out = render(&outlines, &BTreeSet::new(), &BTreeMap::new(), 300);
        assert!(out.chars().count() <= 400, "{}", out.chars().count());
        assert!(out.contains("Repository symbol map"));
    }
}
