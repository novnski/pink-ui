//! Labels never join facts with separators (Pink UI rethink decision 31,
//! novnski/pink#173): a primary line and a quieter secondary line, chips,
//! icons or a plain phrase instead (`main in my-worktree`, `{model} on
//! {provider}`, `2 of 2 turns active, 3 waiting`, `attempt 2 of 4`).
//!
//! Scope: every string literal in the crate's Rust sources (`src/` and the
//! web player's `web/src/`), which covers visible copy, tooltips,
//! accessibility names, `DESCRIPTION`s, stories and sample data. Comments
//! and char literals are not labels and are skipped. A literal fails when it
//! holds a middle dot `·`, an em or en dash, ` -- `, ` | `, ` / `, or a
//! number fraction like `2/4` (write `2 of 4`). This is Pink's
//! `scripts/ui-ratchet.py` joiner rule plus the slash.
//!
//! Content that is not a label (a shell command, a markdown converter's
//! dash) is allowed by a reviewed comment on the literal's first line or the
//! line above it: `// joiners: allow <reason>`. The reason is required.

use std::{fs, path::Path};

/// String literals in Rust `source`, with the 1-based line each starts on.
fn literals(source: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    let at = |i: usize, s: &str| {
        s.chars()
            .enumerate()
            .all(|(k, c)| chars.get(i + k) == Some(&c))
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if at(i, "//") {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if at(i, "/*") {
            i += 2;
            while i < chars.len() && !at(i, "*/") {
                line += usize::from(chars[i] == '\n');
                i += 1;
            }
            i += 2;
        } else if (c == 'r' || (c == 'b' && chars.get(i + 1) == Some(&'r')))
            && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_'))
            && {
                let mut j = i + 1 + usize::from(c == 'b');
                while chars.get(j) == Some(&'#') {
                    j += 1;
                }
                chars.get(j) == Some(&'"')
            }
        {
            // Raw string: r"..", r#".."#.
            let mut j = i + 1 + usize::from(c == 'b');
            let mut hashes = 0;
            while chars[j] == '#' {
                hashes += 1;
                j += 1;
            }
            j += 1;
            let close: String = std::iter::once('"')
                .chain((0..hashes).map(|_| '#'))
                .collect();
            let start = j;
            while !at(j, &close) {
                j += 1;
            }
            let text: String = chars[start..j].iter().collect();
            out.push((line, text.clone()));
            line += text.matches('\n').count();
            i = j + close.chars().count();
        } else if c == '"' {
            let mut j = i + 1;
            let mut text = String::new();
            while chars[j] != '"' {
                if chars[j] == '\\' {
                    text.push(chars[j]);
                    j += 1;
                }
                text.push(chars[j]);
                j += 1;
            }
            out.push((line, text.clone()));
            line += text.matches('\n').count();
            i = j + 1;
        } else if c == '\'' {
            // A char literal ('·', '\n', '\u{b7}') is not a label; a lifetime
            // ('a) has no closing quote within reach.
            let close = (i + 2..(i + 12).min(chars.len())).find(|&j| chars[j] == '\'');
            match close {
                Some(j) if chars[i + 1] == '\\' || j == i + 2 => i = j + 1,
                _ => i += 1,
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The joiner a literal uses, if any.
fn joiner(text: &str) -> Option<&'static str> {
    if text.contains('·') {
        return Some("middle dot");
    }
    if text.contains('—') || text.contains('–') {
        return Some("em or en dash");
    }
    for (needle, name) in [(" -- ", "double hyphen"), (" | ", "bar"), (" / ", "slash")] {
        if text.contains(needle) {
            return Some(name);
        }
    }
    let bytes = text.as_bytes();
    let fraction = (1..bytes.len().saturating_sub(1)).any(|k| {
        bytes[k] == b'/' && bytes[k - 1].is_ascii_digit() && bytes[k + 1].is_ascii_digit()
    });
    fraction.then_some("number fraction (write `2 of 4`)")
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_lexer_finds_labels_and_skips_comments() {
    let src = "// a · b\nlet x = \"main · dev\"; /* c – d */ let y = '·';\nlet z = r#\"1/3\"#;\nfn f<'a>(s: &'a str) {}\n";
    let found: Vec<_> = literals(src)
        .into_iter()
        .filter(|(_, text)| joiner(text).is_some())
        .collect();
    assert_eq!(
        found,
        [(2, "main · dev".to_string()), (3, "1/3".to_string())]
    );
    for clean in [
        "main in my-worktree",
        "owner/repo#101",
        "Pull request #12, Open",
        "⌘/",
    ] {
        assert_eq!(joiner(clean), None, "{clean}");
    }
}

#[test]
fn no_label_joins_facts_with_a_separator() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "web/src"] {
        rust_files(&root.join(dir), &mut files);
    }
    files.sort();
    let mut failures = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        let lines: Vec<&str> = source.lines().collect();
        for (line, text) in literals(&source) {
            let Some(kind) = joiner(&text) else { continue };
            let allowed = [line.checked_sub(2), Some(line - 1)]
                .into_iter()
                .flatten()
                .filter_map(|ix| lines[ix].split_once("// joiners: allow"))
                .any(|(_, reason)| !reason.trim().is_empty());
            if !allowed {
                let shown: String = text.chars().take(70).collect();
                failures.push(format!(
                    "{}:{line}: {kind}: {shown:?}",
                    file.strip_prefix(root).unwrap().display()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} labels join facts with a separator; use hierarchy, a chip or a plain phrase \
         (or `// joiners: allow <reason>` for content that isn't a label):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
