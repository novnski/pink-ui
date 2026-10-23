//! The component catalog as JSON, for the showcase site.
//!
//! The site's build script calls [`crate::catalog`] from Node, so names,
//! categories and descriptions come from the same registry as the gallery and
//! can't drift apart.

use pink_ui::components::{CATEGORIES, COMPONENTS, Kind};

/// A JSON string literal.
fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn json() -> String {
    let categories: Vec<String> = CATEGORIES.iter().map(|c| string(c)).collect();
    let components: Vec<String> = COMPONENTS
        .iter()
        .map(|c| {
            let kind = match c.kind {
                Kind::Core => "core",
                Kind::Pink => "pink",
            };
            format!(
                "{{\"slug\":{},\"name\":{},\"category\":{},\"description\":{},\"kind\":{},\"source\":{}}}",
                string(c.slug),
                string(c.name),
                string(c.category),
                string(c.description),
                string(kind),
                string(&format!("src/components/{}.rs", c.slug.replace('-', "_"))),
            )
        })
        .collect();
    format!(
        "{{\"categories\":[{}],\"components\":[{}]}}",
        categories.join(","),
        components.join(",")
    )
}
