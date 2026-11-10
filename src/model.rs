//! Page model: a page is a list of blocks. Blocks are parsed from and serialized to the
//! Markdown dialect understood by `page/index.js`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Heading,
    Text,
    Figure,
    Table,
    Sheet,
    Math,
    Code,
    Desmos,
    Map,
    Model,
    Infobox,
    Callout,
    Footnote,
    Raw,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Heading => "Heading",
            Kind::Text => "Text",
            Kind::Figure => "Image / video",
            Kind::Table => "Table",
            Kind::Sheet => "Interactive sheet",
            Kind::Math => "LaTeX",
            Kind::Code => "Code window",
            Kind::Desmos => "Desmos graph",
            Kind::Map => "Map",
            Kind::Model => "3D model",
            Kind::Infobox => "Infobox",
            Kind::Callout => "Callout",
            Kind::Footnote => "Footnote",
            Kind::Raw => "Raw Markdown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Block {
    pub id: u64,
    pub kind: Kind,
    /// Main body: heading text, Markdown, code, LaTeX, ...
    pub text: String,
    pub attrs: BTreeMap<String, String>,
    /// Cells for `Table` and `Sheet` blocks.
    pub grid: Vec<Vec<String>>,
}

impl Block {
    pub fn new(kind: Kind) -> Self {
        let mut b = Block {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            kind,
            text: String::new(),
            attrs: BTreeMap::new(),
            grid: Vec::new(),
        };
        match kind {
            Kind::Heading => {
                b.attrs.insert("level".into(), "2".into());
                b.text = "New section".into();
            }
            Kind::Text => b.text = "Write something here. Use **bold**, *italic*, [[Another page]] and [a link](https://example.com).".into(),
            Kind::Figure => {
                b.attrs.insert("align".into(), "right".into());
                b.attrs.insert("width".into(), "300".into());
            }
            Kind::Table => {
                b.grid = vec![
                    vec!["Column A".into(), "Column B".into(), "Column C".into()],
                    vec!["1".into(), "2".into(), "3".into()],
                    vec!["4".into(), "5".into(), "6".into()],
                ];
            }
            Kind::Sheet => {
                b.grid = vec![
                    vec!["Item".into(), "Qty".into(), "Price".into(), "Total".into()],
                    vec!["Apples".into(), "4".into(), "0.5".into(), "=B2*C2".into()],
                    vec!["Pears".into(), "6".into(), "0.75".into(), "=B3*C3".into()],
                    vec!["Total".into(), "=SUM(B2:B3)".into(), "".into(), "=SUM(D2:D3)".into()],
                ];
            }
            Kind::Math => b.text = "E = mc^2".into(),
            Kind::Code => {
                b.attrs.insert("lang".into(), "python".into());
                b.text = "print(\"Hello, world!\")".into();
            }
            Kind::Desmos => b.text = "y=\\sin\\left(x\\right)\ny=a x^2\na=1".into(),
            Kind::Map => b.text = "center: 40.7128, -74.0060\nzoom: 11\nmarker: 40.7128, -74.0060 | New York".into(),
            Kind::Model => {
                b.attrs.insert("src".into(), "https://modelviewer.dev/shared-assets/models/Astronaut.glb".into());
                b.attrs.insert("alt".into(), "A 3D model".into());
                b.attrs.insert("height".into(), "420".into());
            }
            Kind::Infobox => b.text = "title: Subject\nType: Example\nFounded: 2024".into(),
            Kind::Callout => {
                b.attrs.insert("variant".into(), "note".into());
                b.text = "A highlighted note.".into();
            }
            Kind::Footnote => b.text = "Footnote text.".into(),
            Kind::Raw => b.text = "<!-- raw Markdown goes here -->".into(),
        }
        b
    }

    pub fn duplicate(&self) -> Block {
        let mut b = self.clone();
        b.id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        b
    }

    pub fn attr(&self, key: &str) -> &str {
        self.attrs.get(key).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn set_attr(&mut self, key: &str, value: impl Into<String>) {
        let v = value.into();
        if v.is_empty() {
            self.attrs.remove(key);
        } else {
            self.attrs.insert(key.to_string(), v);
        }
    }
}

// ---------------------------------------------------------------- attribute strings

/// Parses `key=value key="a value" flag`.
pub fn parse_attrs(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-') {
            i += 1;
        }
        if start == i {
            i += 1;
            continue;
        }
        let key: String = chars[start..i].iter().collect();
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            let mut val = String::new();
            if i < chars.len() && chars[i] == '"' {
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    val.push(chars[i]);
                    i += 1;
                }
                i += 1;
            } else {
                while i < chars.len() && !chars[i].is_whitespace() {
                    val.push(chars[i]);
                    i += 1;
                }
            }
            out.insert(key, val);
        } else {
            out.insert(key, "1".to_string());
        }
    }
    out
}

fn format_attrs(attrs: &BTreeMap<String, String>, order: &[&str]) -> String {
    let mut parts = Vec::new();
    let mut emit = |k: &str, v: &str| {
        if v.is_empty() {
            return;
        }
        let v = v.replace('"', "'").replace('\n', " ");
        if v.chars().all(|c| c.is_alphanumeric() || "#._-/:%".contains(c)) {
            parts.push(format!("{k}={v}"));
        } else {
            parts.push(format!("{k}=\"{v}\""));
        }
    };
    for k in order {
        if let Some(v) = attrs.get(*k) {
            emit(k, v);
        }
    }
    for (k, v) in attrs {
        if !order.contains(&k.as_str()) {
            emit(k, v);
        }
    }
    parts.join(" ")
}

// ---------------------------------------------------------------- CSV / tables

pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    cur.push('"');
                    i += 1;
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' && cur.is_empty() {
            in_quotes = true;
        } else if ch == ',' {
            row.push(std::mem::take(&mut cur));
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            row.push(std::mem::take(&mut cur));
            rows.push(std::mem::take(&mut row));
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    while rows.last().map(|r| r.iter().all(|c| c.is_empty())).unwrap_or(false) {
        rows.pop();
    }
    rectangular(&mut rows);
    rows
}

/// Quotes only what must be quoted so that formulas such as `=IF(A1>1;"y";"n")` stay readable.
pub fn to_csv(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|c| {
                    if c.contains(',') || c.contains('\n') || c.starts_with('"') {
                        format!("\"{}\"", c.replace('"', "\"\""))
                    } else {
                        c.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn rectangular(rows: &mut Vec<Vec<String>>) {
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    for r in rows.iter_mut() {
        while r.len() < w {
            r.push(String::new());
        }
    }
}

fn split_table_row(line: &str) -> Vec<String> {
    let mut s = line.trim();
    s = s.strip_prefix('|').unwrap_or(s);
    if s.ends_with('|') && !s.ends_with("\\|") {
        s = &s[..s.len() - 1];
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'|') {
            cur.push('|');
            i += 2;
            continue;
        }
        if chars[i] == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(chars[i]);
        }
        i += 1;
    }
    cells.push(cur.trim().to_string());
    cells
}

fn is_table_sep(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && t.contains('|')
        && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
        && t.contains('-')
}

// ---------------------------------------------------------------- parse

fn starts_special(line: &str) -> bool {
    let t = line.trim_start();
    line.starts_with("```")
        || line.starts_with(":::")
        || line.starts_with("$$")
        || (line.starts_with('#') && line.trim_start_matches('#').starts_with(' '))
        || (t.starts_with("[^") && t.contains("]:"))
        || line.starts_with('|')
}

pub fn parse_markdown(md: &str) -> Vec<Block> {
    let md = md.replace("\r\n", "\n");
    let lines: Vec<&str> = md.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // fenced block
        if let Some(info) = line.strip_prefix("```") {
            let info = info.trim();
            let (lang, rest) = match info.split_once(char::is_whitespace) {
                Some((l, r)) => (l.to_lowercase(), r.to_string()),
                None => (info.to_lowercase(), String::new()),
            };
            let attrs = parse_attrs(&rest);
            let mut body = Vec::new();
            i += 1;
            while i < lines.len() && lines[i].trim_end() != "```" {
                body.push(lines[i]);
                i += 1;
            }
            i += 1;
            let body = body.join("\n");
            blocks.push(fence_to_block(&lang, attrs, body));
            continue;
        }

        // directive
        if let Some(rest) = line.strip_prefix(":::") {
            let rest = rest.trim();
            let (name, attr_text) = match rest.split_once(char::is_whitespace) {
                Some((n, a)) => (n.to_lowercase(), a.to_string()),
                None => (rest.to_lowercase(), String::new()),
            };
            let mut body = Vec::new();
            let mut depth = 1;
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                if l.starts_with(":::") && l[3..].trim().is_empty() {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                } else if l.starts_with(":::") {
                    depth += 1;
                }
                body.push(l);
                i += 1;
            }
            i += 1;
            blocks.push(directive_to_block(&name, parse_attrs(&attr_text), body.join("\n")));
            continue;
        }

        // display math
        if line.starts_with("$$") {
            let trimmed = line.trim();
            if trimmed.len() > 4 && trimmed.ends_with("$$") {
                let mut b = Block::new(Kind::Math);
                b.text = trimmed[2..trimmed.len() - 2].trim().to_string();
                blocks.push(b);
                i += 1;
                continue;
            }
            let mut body = Vec::new();
            let first = trimmed.trim_start_matches('$');
            if !first.trim().is_empty() {
                body.push(first.trim().to_string());
            }
            i += 1;
            while i < lines.len() && !lines[i].trim_end().ends_with("$$") {
                body.push(lines[i].to_string());
                i += 1;
            }
            if i < lines.len() {
                let last = lines[i].trim_end().trim_end_matches('$');
                if !last.trim().is_empty() {
                    body.push(last.to_string());
                }
                i += 1;
            }
            let mut b = Block::new(Kind::Math);
            b.text = body.join("\n").trim().to_string();
            blocks.push(b);
            continue;
        }

        // heading
        if line.starts_with('#') {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            if hashes <= 6 && line[hashes..].starts_with(' ') {
                let mut b = Block::new(Kind::Heading);
                b.attrs.insert("level".into(), hashes.to_string());
                b.text = line[hashes..].trim().trim_end_matches('#').trim().to_string();
                blocks.push(b);
                i += 1;
                continue;
            }
        }

        // footnote definition
        let t = line.trim_start();
        if t.starts_with("[^") {
            if let Some(end) = t.find("]:") {
                let id = &t[2..end];
                if !id.is_empty() && !id.contains(char::is_whitespace) {
                    let mut text = t[end + 2..].trim().to_string();
                    i += 1;
                    while i < lines.len() && lines[i].starts_with("  ") && !lines[i].trim().is_empty() {
                        text.push(' ');
                        text.push_str(lines[i].trim());
                        i += 1;
                    }
                    let mut b = Block::new(Kind::Footnote);
                    b.attrs.insert("id".into(), id.to_string());
                    b.text = text;
                    blocks.push(b);
                    continue;
                }
            }
        }

        // table
        if line.contains('|') && i + 1 < lines.len() && is_table_sep(lines[i + 1]) {
            let mut grid = vec![split_table_row(line)];
            i += 2;
            while i < lines.len() && !lines[i].trim().is_empty() && lines[i].contains('|') {
                grid.push(split_table_row(lines[i]));
                i += 1;
            }
            rectangular(&mut grid);
            let mut b = Block::new(Kind::Table);
            b.grid = grid;
            blocks.push(b);
            continue;
        }

        // paragraph / list / quote: keep as Markdown text
        let mut para = vec![line];
        i += 1;
        while i < lines.len() && !lines[i].trim().is_empty() && !starts_special(lines[i]) {
            para.push(lines[i]);
            i += 1;
        }
        let mut b = Block::new(Kind::Text);
        b.text = para.join("\n");
        blocks.push(b);
    }
    blocks
}

fn fence_to_block(lang: &str, attrs: BTreeMap<String, String>, body: String) -> Block {
    let (kind, key) = match lang {
        "sheet" => (Kind::Sheet, ""),
        "map" => (Kind::Map, ""),
        "desmos" => (Kind::Desmos, ""),
        "model" => (Kind::Model, ""),
        "math" | "latex" => (Kind::Math, ""),
        "py" => (Kind::Code, "python"),
        "js" => (Kind::Code, "javascript"),
        "rs" => (Kind::Code, "rust"),
        "c++" => (Kind::Code, "cpp"),
        _ => (Kind::Code, lang),
    };
    let mut b = Block::new(kind);
    b.attrs.clear();
    b.text = body;
    for (k, v) in attrs {
        b.attrs.insert(k, v);
    }
    match kind {
        Kind::Code => {
            let l = if key.is_empty() { "text" } else { key };
            b.attrs.insert("lang".into(), l.to_string());
        }
        Kind::Sheet => {
            b.grid = parse_csv(&b.text);
            b.text.clear();
        }
        _ => {}
    }
    b
}

fn directive_to_block(name: &str, attrs: BTreeMap<String, String>, body: String) -> Block {
    match name {
        "text" => {
            let mut b = Block::new(Kind::Text);
            b.attrs = attrs;
            b.text = body;
            b
        }
        "figure" => {
            let mut b = Block::new(Kind::Figure);
            b.attrs = attrs;
            b
        }
        "infobox" => {
            let mut b = Block::new(Kind::Infobox);
            b.text = body;
            b
        }
        "note" | "tip" | "warning" => {
            let mut b = Block::new(Kind::Callout);
            b.attrs.insert("variant".into(), name.to_string());
            b.text = body;
            b
        }
        _ => {
            let mut b = Block::new(Kind::Raw);
            b.text = format!("::: {name} {}\n{body}\n:::", format_attrs(&attrs, &[]));
            b
        }
    }
}

// ---------------------------------------------------------------- serialize

pub fn to_markdown(blocks: &[Block]) -> String {
    let mut out: Vec<String> = Vec::new();
    for b in blocks {
        let s = match b.kind {
            Kind::Heading => {
                let level: usize = b.attr("level").parse().unwrap_or(2).clamp(1, 6);
                format!("{} {}", "#".repeat(level), b.text.trim())
            }
            Kind::Text => {
                let style = format_attrs(
                    &b.attrs.iter().filter(|(k, _)| matches!(k.as_str(), "font" | "size" | "color" | "align")).map(|(k, v)| (k.clone(), v.clone())).collect(),
                    &["font", "size", "color", "align"],
                );
                if style.is_empty() {
                    b.text.trim_end().to_string()
                } else {
                    format!("::: text {style}\n{}\n:::", b.text.trim_end())
                }
            }
            Kind::Figure => format!("::: figure {}\n:::", format_attrs(&b.attrs, &["src", "caption", "align", "width", "alt"])),
            Kind::Table => {
                let mut grid = b.grid.clone();
                rectangular(&mut grid);
                let row = |r: &Vec<String>| format!("| {} |", r.iter().map(|c| c.replace('|', "\\|").replace('\n', " ")).collect::<Vec<_>>().join(" | "));
                let w = grid[0].len();
                let mut lines = vec![row(&grid[0]), format!("|{}|", vec![" --- "; w].join("|"))];
                for r in grid.iter().skip(1) {
                    lines.push(row(r));
                }
                lines.join("\n")
            }
            Kind::Sheet => {
                let mut grid = b.grid.clone();
                rectangular(&mut grid);
                let a = format_attrs(&b.attrs, &["height"]);
                format!("```sheet{}\n{}\n```", if a.is_empty() { String::new() } else { format!(" {a}") }, to_csv(&grid))
            }
            Kind::Math => format!("$$\n{}\n$$", b.text.trim()),
            Kind::Code => {
                let lang = if b.attr("lang").is_empty() { "text" } else { b.attr("lang") };
                let mut info = lang.to_string();
                if b.attr("canvas") == "1" {
                    info.push_str(" canvas");
                }
                format!("```{info}\n{}\n```", b.text.trim_end_matches('\n'))
            }
            Kind::Desmos => fence("desmos", &b.attrs, &["height"], &b.text),
            Kind::Map => fence("map", &b.attrs, &["height"], &b.text),
            Kind::Model => fence("model", &b.attrs, &["src", "alt", "height", "poster", "rotate"], ""),
            Kind::Infobox => format!("::: infobox\n{}\n:::", b.text.trim()),
            Kind::Callout => {
                let v = match b.attr("variant") {
                    "tip" => "tip",
                    "warning" => "warning",
                    _ => "note",
                };
                format!("::: {v}\n{}\n:::", b.text.trim())
            }
            Kind::Footnote => format!("[^{}]: {}", b.attr("id"), b.text.trim().replace('\n', " ")),
            Kind::Raw => b.text.trim_end().to_string(),
        };
        out.push(s);
    }
    let mut s = out.join("\n\n");
    s.push('\n');
    s
}

fn fence(name: &str, attrs: &BTreeMap<String, String>, order: &[&str], body: &str) -> String {
    let a = format_attrs(attrs, order);
    let head = if a.is_empty() { format!("```{name}") } else { format!("```{name} {a}") };
    format!("{head}\n{}\n```", body.trim_end_matches('\n'))
}

/// First level-1 heading of a Markdown file, if any.
pub fn extract_title(md: &str) -> Option<String> {
    let mut in_fence = false;
    for line in md.lines() {
        if line.starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence {
            if let Some(t) = line.strip_prefix("# ") {
                return Some(t.trim().to_string());
            }
        }
    }
    None
}

pub fn next_footnote_id(blocks: &[Block]) -> String {
    let max = blocks
        .iter()
        .filter(|b| b.kind == Kind::Footnote)
        .filter_map(|b| b.attr("id").parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (max + 1).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_showcase() {
        let md = include_str!("../assets/showcase.md");
        let blocks = parse_markdown(md);
        let again = to_markdown(&blocks);
        let blocks2 = parse_markdown(&again);
        assert_eq!(blocks.len(), blocks2.len());
        for (a, b) in blocks.iter().zip(blocks2.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.text.trim(), b.text.trim());
            assert_eq!(a.attrs, b.attrs);
            assert_eq!(a.grid, b.grid);
        }
        assert_eq!(again, to_markdown(&blocks2));
    }

    #[test]
    fn kinds_present() {
        let blocks = parse_markdown(include_str!("../assets/showcase.md"));
        for k in [Kind::Heading, Kind::Text, Kind::Figure, Kind::Table, Kind::Sheet, Kind::Math, Kind::Code, Kind::Desmos, Kind::Map, Kind::Model, Kind::Infobox, Kind::Callout, Kind::Footnote] {
            assert!(blocks.iter().any(|b| b.kind == k), "missing {:?}", k);
        }
        assert_eq!(blocks.iter().filter(|b| b.kind == Kind::Code && b.attr("canvas") == "1").count(), 6);
    }

    #[test]
    fn attrs() {
        let a = parse_attrs("src=\"media/a b.png\" width=200 canvas");
        assert_eq!(a["src"], "media/a b.png");
        assert_eq!(a["width"], "200");
        assert_eq!(a["canvas"], "1");
    }

    #[test]
    fn csv_formulas() {
        let rows = parse_csv("a,b\n=IF(A1>1;\"y\";\"n\"),\"x,y\"");
        assert_eq!(rows[1][0], "=IF(A1>1;\"y\";\"n\")");
        assert_eq!(rows[1][1], "x,y");
        assert_eq!(parse_csv(&to_csv(&rows)), rows);
    }
}
