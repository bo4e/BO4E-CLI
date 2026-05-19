// crates/bo4e-cli/src/completion/marker.rs

const MARKER_OPEN: &str = ">>> bo4e completion >>>";
const MARKER_CLOSE: &str = "<<< bo4e completion <<<";

/// Splice a fresh `body` between marker lines inside `original`. If no
/// existing marker block is present, append a new one at the end. Returns
/// the new file contents.
pub fn splice(original: &str, body: &str, comment_leader: &str) -> String {
    let open = format!("{comment_leader} {MARKER_OPEN}");
    let close = format!("{comment_leader} {MARKER_CLOSE}");
    if let Some(block) = find_block(original, &open, &close) {
        let mut out = String::with_capacity(original.len() + body.len());
        out.push_str(&original[..block.start]);
        out.push_str(&open);
        out.push('\n');
        out.push_str(body.trim_end());
        out.push('\n');
        out.push_str(&close);
        out.push_str(&original[block.end..]);
        out
    } else {
        let needs_nl = !original.is_empty() && !original.ends_with('\n');
        format!(
            "{}{}\n{}\n{}\n{}\n",
            original,
            if needs_nl { "\n" } else { "" },
            &open,
            body.trim_end(),
            &close,
        )
    }
}

/// Remove the marker block from `original`, leaving a single newline gap if
/// the block was sandwiched between content lines. Returns `(new_contents,
/// was_present)`.
pub fn strip(original: &str, comment_leader: &str) -> (String, bool) {
    let open = format!("{comment_leader} {MARKER_OPEN}");
    let close = format!("{comment_leader} {MARKER_CLOSE}");
    match find_block(original, &open, &close) {
        Some(block) => {
            let mut out = String::with_capacity(original.len());
            out.push_str(&original[..block.start]);
            out.push_str(&original[block.end..]);
            (out, true)
        }
        None => (original.to_string(), false),
    }
}

/// Test for marker-block presence without modifying the file.
pub fn is_installed(original: &str, comment_leader: &str) -> bool {
    let open = format!("{comment_leader} {MARKER_OPEN}");
    let close = format!("{comment_leader} {MARKER_CLOSE}");
    find_block(original, &open, &close).is_some()
}

struct Block { start: usize, end: usize }

fn find_block(s: &str, open: &str, close: &str) -> Option<Block> {
    let start = s.find(open)?;
    let after_open = start + open.len();
    let rel_close_offset = s[after_open..].find(close)?;
    let close_pos = after_open + rel_close_offset;
    let end = close_pos + close.len();
    let line_start = s[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line_end = s[end..].find('\n').map(|p| end + p + 1).unwrap_or(s.len());
    Some(Block { start: line_start, end: line_end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splice_appends_when_marker_absent() {
        let out = splice("existing rc\n", "do_a_thing", "#");
        assert!(out.contains("# >>> bo4e completion >>>"));
        assert!(out.contains("do_a_thing"));
        assert!(out.contains("# <<< bo4e completion <<<"));
        assert!(out.starts_with("existing rc\n"));
    }

    #[test]
    fn splice_appends_newline_when_input_does_not_end_with_one() {
        let out = splice("no newline", "body", "#");
        assert!(out.starts_with("no newline\n"));
    }

    #[test]
    fn splice_replaces_existing_block() {
        let initial = "before\n# >>> bo4e completion >>>\nOLD\n# <<< bo4e completion <<<\nafter\n";
        let out = splice(initial, "NEW", "#");
        assert!(out.contains("NEW"));
        assert!(!out.contains("OLD"));
        assert!(out.contains("before"));
        assert!(out.contains("after"));
    }

    #[test]
    fn strip_removes_block() {
        let initial = "a\n# >>> bo4e completion >>>\nbody\n# <<< bo4e completion <<<\nb\n";
        let (out, present) = strip(initial, "#");
        assert!(present);
        assert_eq!(out, "a\nb\n");
    }

    #[test]
    fn strip_noop_when_block_absent() {
        let (out, present) = strip("plain rc\n", "#");
        assert!(!present);
        assert_eq!(out, "plain rc\n");
    }

    #[test]
    fn is_installed_detects_present_block() {
        let s = "# >>> bo4e completion >>>\nx\n# <<< bo4e completion <<<\n";
        assert!(is_installed(s, "#"));
        assert!(!is_installed("nothing here", "#"));
    }
}
