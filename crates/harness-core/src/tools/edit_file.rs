//! `edit_file` — surgical string replacement with the spec §7 match ladder:
//!
//! 1. **exact** — `old_string` matches uniquely;
//! 2. **line-range hint** — multiple exact matches disambiguated by the
//!    occurrence nearest `line_hint`;
//! 3. **fuzzy** — best sliding line-window by normalized Levenshtein
//!    similarity ≥ threshold (typos / drift tolerated).
//!
//! Read-before-edit is enforced via [`FileStateTracker`], and stale reads
//! (file changed since) force a re-read.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;

use super::{Tool, ToolCtx, ToolError, ToolOutput, truncate_with_tempfile, unified_diff};
use strsim::normalized_levenshtein;

/// Minimum normalized similarity for the fuzzy rung.
const FUZZY_THRESHOLD: f64 = 0.85;
/// How far (in lines) a hint may be from a match to count as "nearest".
pub(crate) const PREVIEW_DIFF_CHARS: usize = 1_600;

#[derive(Debug)]
pub struct EditFileTool;

#[derive(Debug)]
struct Replacement {
    new_content: String,
    rung: &'static str,
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Replace an exact snippet in a file. If the snippet matches several \
         times pass `line_hint` (1-based line near the intended spot); if it \
         doesn't match exactly, close variants are found fuzzily. The file \
         must have been read first."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path relative to project root (or absolute)."},
                "old_string": {"type": "string", "description": "Exact text to replace (include surrounding context to make it unique)."},
                "new_string": {"type": "string", "description": "Replacement text."},
                "line_hint": {"type": "integer", "description": "Optional 1-based line number near the intended match."}
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn concurrency_safe(&self) -> bool {
        false
    }

    fn approval_preview(&self, input: &Value, ctx: &ToolCtx) -> Option<String> {
        let path = input.get("path")?.as_str()?;
        let resolved = ctx.resolve(Path::new(path));
        let disp = super::write_file::rel(&resolved, &ctx.project_root);
        let old = String::from_utf8_lossy(&std::fs::read(&resolved).ok()?).into_owned();
        let old_snip = input.get("old_string")?.as_str()?;
        let new_snip = input.get("new_string")?.as_str()?;
        // Preview against the unique-exact outcome; ladder may differ at run
        // time but this is what the human is approving conceptually.
        let new_full = old.replacen(old_snip, new_snip, 1);
        let d = unified_diff(&old, &new_full, &disp);
        let mut out: String = d.chars().take(PREVIEW_DIFF_CHARS).collect();
        if out.chars().count() == PREVIEW_DIFF_CHARS {
            out.push_str("\n… (diff truncated)");
        }
        Some(out)
    }

    async fn run(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        let obj = input.as_object().ok_or_else(|| ToolError::InvalidInput {
            tool: "edit_file",
            problem: "input must be an object".into(),
        })?;
        let raw_path =
            obj.get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidInput {
                    tool: "edit_file",
                    problem: "`path` must be a string".into(),
                })?;
        let old_s = obj
            .get("old_string")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "edit_file",
                problem: "`old_string` must be a non-empty string".into(),
            })?;
        let new_s = obj
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidInput {
                tool: "edit_file",
                problem: "`new_string` must be a string".into(),
            })?;
        let line_hint = obj
            .get("line_hint")
            .and_then(Value::as_u64)
            .map(|v| v as usize);

        let resolved = ctx.resolve(Path::new(raw_path));
        let disp = super::write_file::rel(&resolved, &ctx.project_root);
        if !resolved.exists() {
            return Ok(ToolOutput::failure(
                format!("ERROR: {disp} does not exist — use write_file to create files"),
                format!("edit_file: missing {disp}"),
            ));
        }
        ctx.require_read_for_mutation("edit_file", &resolved, true)?;
        // Rewind support: stash the pre-edit image before touching disk.
        ctx.checkpoint_before_mutation(&resolved);

        let bytes = tokio::fs::read(&resolved)
            .await
            .map_err(|e| ToolError::Failed(format!("read {}: {e}", resolved.display())))?;
        // Strict UTF-8: lossy conversion would silently corrupt binary
        // files by replacing invalid bytes with U+FFFD on write-back.
        let current = String::from_utf8(bytes).map_err(|_| {
            ToolError::Failed(format!(
                "{disp} is not valid UTF-8 text; refusing to edit it"
            ))
        })?;

        let rep = apply_ladder(&current, old_s, new_s, line_hint).map_err(|msg| {
            // Model-visible guidance so it can adjust (never crash).
            ToolError::Failed(msg)
        })?;

        super::atomic_write(&resolved, rep.new_content.as_bytes())
            .await
            .map_err(|e| ToolError::Failed(format!("write {disp}: {e}")))?;
        ctx.note_read(&resolved);

        let diff = unified_diff(&current, &rep.new_content, &disp);
        let body = format!("{disp}: edited (match: {})\n{diff}", rep.rung);
        let result = truncate_with_tempfile(&body, ctx);
        // Reviewer journal entry (spec section 9 v0.9).
        if let Ok(mut j) = ctx.edit_journal.lock() {
            j.push(result.clone());
        }
        tracing::debug!(path = %disp, rung = rep.rung, "edit_file done");
        Ok(ToolOutput::success(
            result,
            format!("edited {disp} ({})", rep.rung),
        ))
    }
}

/// Pure ladder logic over in-memory text — exhaustively unit-testable.
fn apply_ladder(
    content: &str,
    old: &str,
    new: &str,
    line_hint: Option<usize>,
) -> Result<Replacement, String> {
    let occurrences: Vec<usize> = content.match_indices(old).map(|(i, _)| i).collect();

    match occurrences.len() {
        1 => {
            let mut c = String::with_capacity(content.len());
            let (pre, post) = (
                &content[..occurrences[0]],
                &content[occurrences[0] + old.len()..],
            );
            c.push_str(pre);
            c.push_str(new);
            c.push_str(post);
            Ok(Replacement {
                new_content: c,
                rung: "exact",
            })
        }
        n if n > 1 => {
            let Some(hint) = line_hint else {
                return Err(format!(
                    "old_string matches {n} places; include more surrounding context or pass line_hint"
                ));
            };
            // Nearest occurrence to the hint line wins.
            let best = occurrences
                .iter()
                .map(|&pos| (line_of(content, pos), pos))
                .min_by_key(|(line, _)| line.abs_diff(hint))
                .map(|(_, pos)| pos)
                .expect("non-empty");
            let mut c = String::with_capacity(content.len());
            c.push_str(&content[..best]);
            c.push_str(new);
            c.push_str(&content[best + old.len()..]);
            Ok(Replacement {
                new_content: c,
                rung: "exact+hint",
            })
        }
        0 => fuzzy_replace(content, old, new, line_hint),
        _ => unreachable!(),
    }
}

fn fuzzy_replace(
    content: &str,
    old: &str,
    new: &str,
    line_hint: Option<usize>,
) -> Result<Replacement, String> {
    let k = old.lines().count().max(1);
    let old_norm = old.trim_end_matches('\n');
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < k {
        return Err(format!(
            "no match: file has fewer lines ({}) than old_string ({k}) and no exact match",
            lines.len()
        ));
    }

    let score_at = |start: usize| -> f64 {
        normalized_levenshtein(&lines[start..start + k].join("\n"), old_norm)
    };

    // Best-scoring window overall.
    let mut best: Option<(f64, usize)> = None;
    for start in 0..=(lines.len() - k) {
        let score = score_at(start);
        if best.is_none_or(|(bs, _)| score > bs) {
            best = Some((score, start));
        }
    }
    let Some((mut best_score, mut start)) = best else {
        unreachable!("at least one window exists")
    };

    // A hint wins ties: prefer its window unless it is clearly worse.
    if let Some(hint) = line_hint {
        let h0 = hint.saturating_sub(1);
        if h0 + k <= lines.len() {
            let hs = score_at(h0);
            if hs >= FUZZY_THRESHOLD && hs >= best_score - 0.02 {
                best_score = hs;
                start = h0;
            }
        }
    } else {
        // Ambiguity guard: a distant window within 2% of the best ⇒ ask.
        for s in 0..=(lines.len() - k) {
            if s.abs_diff(start) <= 1 {
                continue;
            }
            if score_at(s) >= best_score - 0.02 {
                return Err(
                    "several similar regions match fuzzily; pass line_hint to disambiguate"
                        .to_string(),
                );
            }
        }
    }

    // Absolute floor: even the best window must clear the bar.
    if best_score < FUZZY_THRESHOLD {
        return Err(
            "old_string not found and no close variant (fuzzy ≥ 0.85); re-read the file and copy exact text"
                .to_string(),
        );
    }

    let mut out_lines: Vec<&str> = lines[..start].to_vec();
    out_lines.extend(new.lines());
    out_lines.extend(lines[start + k..].iter().copied());
    let mut new_content = out_lines.join("\n");
    if content.ends_with('\n') && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    Ok(Replacement {
        new_content,
        rung: "fuzzy",
    })
}

fn line_of(text: &str, byte_pos: usize) -> usize {
    text[..byte_pos].bytes().filter(|b| *b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perms::PolicyEngine;
    use std::sync::{Arc, Mutex};

    fn ctx_in(dir: &Path) -> ToolCtx {
        ToolCtx::new(
            dir.to_path_buf(),
            Arc::new(Mutex::new(PolicyEngine::new(vec![]))),
            tempfile::tempdir().unwrap().keep(),
        )
    }

    #[test]
    fn ladder_exact_unique() {
        let r = apply_ladder("a\nb\nc\n", "b", "B", None).unwrap();
        assert_eq!(r.rung, "exact");
        assert_eq!(r.new_content, "a\nB\nc\n");
    }

    #[test]
    fn ladder_multiple_requires_hint() {
        let err = apply_ladder("x\nfoo\ny\nfoo\n", "foo", "F", None).unwrap_err();
        assert!(err.contains("line_hint"));
    }

    #[test]
    fn ladder_hint_picks_nearest_occurrence() {
        let r = apply_ladder("foo\nmid\nfoo\nend\nfoo\n", "foo", "HERE", Some(5)).unwrap();
        assert_eq!(r.rung, "exact+hint");
        assert_eq!(r.new_content, "foo\nmid\nfoo\nend\nHERE\n");
    }

    #[test]
    fn ladder_fuzzy_recovers_close_variant() {
        // model's copy drifted slightly (typo + spacing)
        let old = "pub fn calc(a: u32, b: u32) -> u32 {\n    a - b\n}";
        let content = format!("header\n{old}\ntail\n");
        let drifted = "pub fn calc(a: u32, b: u32) -> u32 {\n    a  + b\n}";
        let r = apply_ladder(&content, drifted, "FIXED", None).unwrap();
        assert_eq!(r.rung, "fuzzy");
        assert!(r.new_content.contains("\nFIXED\n"));
    }

    #[test]
    fn ladder_fuzzy_ambiguous_asks_for_hint() {
        let dup = "alpha beta gamma delta\n";
        let content = format!("{dup}sep\n{dup}");
        // one-char typo: clearly above the fuzzy threshold, matches BOTH copies
        let drifted = "alpha bets gamma delta";
        let err = apply_ladder(&content, drifted, "X", None).unwrap_err();
        assert!(err.contains("line_hint"), "{err}");
    }

    #[test]
    fn ladder_total_miss_is_actionable_error() {
        let err = apply_ladder("one two three\n", "zzz qqq", "X", None).unwrap_err();
        assert!(err.contains("re-read the file"), "{err}");
    }

    #[tokio::test]
    async fn refuses_without_prior_read_and_after_staleness() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("e.txt");
        std::fs::write(&p, "hello world\n").unwrap();

        // no read yet
        let err = EditFileTool
            .run(
                json!({"path": "e.txt", "old_string": "world", "new_string": "there"}),
                &ctx_in(tmp.path()),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("without reading"));

        // read → stale via external change
        let ctx = ctx_in(tmp.path());
        ctx.note_read(&p);
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&p, "changed externally\n").unwrap();
        let err = EditFileTool
            .run(
                json!({"path": "e.txt", "old_string": "x", "new_string": "y"}),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("changed on disk"), "{err}");

        // fresh read → success
        ctx.note_read(&p);
        let out = EditFileTool
            .run(
                json!({"path": "e.txt", "old_string": "external", "new_string": "internal"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.result.contains("-changed externally"));
        assert!(out.result.contains("+changed internally"));
    }

    #[tokio::test]
    async fn end_to_end_edit_updates_file_and_reports_rung() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("m.txt");
        std::fs::write(&p, "one\ntwo\nthree\nfour\nfive\n").unwrap();
        let ctx = ctx_in(tmp.path());
        ctx.note_read(&p);

        let out = EditFileTool
            .run(
                json!({"path": "m.txt", "old_string": "two\nthree\nfour", "new_string": "TWO\nTHREE"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.summary.contains("exact"));
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "one\nTWO\nTHREE\nfive\n"
        );
    }
}
