//! Pure match-ladder algorithm behind `edit_file` — operates on in-memory
//! text only, exhaustively unit-testable.
//!
//! 1. **exact** — `old_string` matches uniquely;
//! 2. **line-range hint** — multiple exact matches disambiguated by the
//!    occurrence nearest `line_hint`;
//! 3. **fuzzy** — best sliding line-window by normalized Levenshtein
//!    similarity ≥ threshold (typos / drift tolerated).

use strsim::normalized_levenshtein;

/// Minimum normalized similarity for the fuzzy rung.
const FUZZY_THRESHOLD: f64 = 0.85;

#[derive(Debug)]
pub(super) struct Replacement {
    pub(super) new_content: String,
    pub(super) rung: &'static str,
}

pub(super) fn apply_ladder(
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
}
