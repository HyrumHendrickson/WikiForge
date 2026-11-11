//! The on-disk site layout.
//!
//! ```text
//! root/
//!   index.html  index.css  index.js        home page
//!   media/                                 images, videos, 3D models
//!   page/index.html index.css index.js     article viewer
//!   pages/*.md                             one Markdown file per article
//!   media/site-logo.*                      the site logo (default: the WikiForge logo)
//!   pages/pages.json                       article list, logo, featured list and cross references
//!                                          (written by the editor)
//! ```

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::extract_title;

const ROOT_HTML: &str = include_str!("../assets/index.html");
const ROOT_CSS: &str = include_str!("../assets/index.css");
const ROOT_JS: &str = include_str!("../assets/index.js");
const PAGE_HTML: &str = include_str!("../assets/page/index.html");
const PAGE_CSS: &str = include_str!("../assets/page/index.css");
const PAGE_JS: &str = include_str!("../assets/page/index.js");

pub const SHOWCASE_MD: &str = include_str!("../assets/showcase.md");
/// The default logo of a new site (and the icon of the editor itself).
pub const DEFAULT_LOGO: &[u8] = include_bytes!("../assets/logo.webp");
const SAMPLE_SVG: &[u8] = include_bytes!("../assets/media/sample.svg");
const SAMPLE_GIF: &[u8] = include_bytes!("../assets/media/sample.gif");
const SAMPLE_WAV: &[u8] = include_bytes!("../assets/media/sample.wav");
const SAMPLE_CSV: &[u8] = include_bytes!("../assets/media/sample-states.csv");

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "webm", "ogv", "mov", "m4v"];
pub const MODEL_EXTS: &[&str] = &["glb", "gltf"];
pub const AUDIO_EXTS: &[&str] = &["mp3", "wav", "ogg", "oga", "m4a", "aac", "flac", "opus"];
pub const CSV_EXTS: &[&str] = &["csv"];

/// Cross references used by the example articles: `Example n` links to each listed number.
/// Chosen so that the site map shows a connected graph with a few cycles and chords.
const EXAMPLE_LINKS: [&[usize]; 10] = [&[2, 3, 5], &[3, 4], &[4, 6], &[5, 7], &[6, 8], &[7, 9], &[8, 10], &[9, 1], &[10, 2], &[1, 4]];

#[derive(Clone)]
pub struct Site {
    pub root: PathBuf,
    pub title: String,
}

#[derive(Clone)]
pub struct PageInfo {
    pub slug: String,
    pub title: String,
    /// Slugs of existing articles this one links to with `[[...]]` (the edges of the site map).
    pub links: Vec<String>,
    /// Featured articles are the ones listed in the sidebar.
    pub featured: bool,
}

pub fn ext_of(path: &str) -> String {
    Path::new(path.split('?').next().unwrap_or(path)).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Mirrors `slugify` in page/index.js so that `[[Page Name]]` links resolve to the right file.
pub fn slugify(name: &str) -> String {
    let kept: String = name.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '_' || *c == '-').collect();
    let s = kept.split_whitespace().collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "section".to_string()
    } else {
        s
    }
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Reads the rest of a JSON string (the opening quote has been consumed) up to the closing quote.
fn read_json_string(chars: &mut std::str::Chars) -> Option<String> {
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or('?'));
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn json_unescape_first(text: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = text.find(&pat)? + pat.len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    read_json_string(&mut rest.chars())
}

/// First `"key": ["a", "b"]` in a JSON text, as a list of strings. `None` when the key is absent.
fn json_string_array(text: &str, key: &str) -> Option<Vec<String>> {
    let pat = format!("\"{key}\"");
    let at = text.find(&pat)? + pat.len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('[')?;
    let mut chars = rest.chars();
    let mut out = Vec::new();
    loop {
        match chars.next()? {
            '"' => out.push(read_json_string(&mut chars)?),
            ']' => return Some(out),
            _ => {}
        }
    }
}

fn json_array(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| format!("\"{}\"", json_escape(s))).collect::<Vec<_>>().join(", "))
}

/// Slugs of the articles that `md` links to with `[[Page]]`, `[[Page|label]]` or `[x](wiki:Page)`.
/// Fenced code (including sheets, maps and so on) is ignored. Order of first appearance, no duplicates.
pub fn extract_links(md: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |name: &str| {
        let name = name.split('#').next().unwrap_or("").trim();
        if name.is_empty() {
            return;
        }
        let slug = slugify(name);
        if seen.insert(slug.clone()) {
            out.push(slug);
        }
    };
    let mut in_fence = false;
    for line in md.lines() {
        if line.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        // text inside `code spans` is not a link
        let mut plain = String::with_capacity(line.len());
        let mut in_code = false;
        for c in line.chars() {
            if c == '`' {
                in_code = !in_code;
            } else if !in_code {
                plain.push(c);
            }
        }
        let line = plain.as_str();
        let mut rest = line;
        while let Some(at) = rest.find("[[") {
            let after = &rest[at + 2..];
            let Some(end) = after.find("]]") else { break };
            push(after[..end].split('|').next().unwrap_or(""));
            rest = &after[end + 2..];
        }
        let mut rest = line;
        while let Some(at) = rest.find("](wiki:") {
            let after = &rest[at + 7..];
            let end = after.find(')').unwrap_or(after.len());
            push(&after[..end]);
            rest = &after[end..];
        }
    }
    out
}

impl Site {
    /// A folder is a valid site when it has the three required entries.
    pub fn is_valid(root: &Path) -> bool {
        root.join("index.html").is_file() && root.join("page").join("index.html").is_file() && root.join("pages").is_dir()
    }

    pub fn open(root: &Path) -> Result<Site, String> {
        if !Self::is_valid(root) {
            return Err(format!("{} is not a WikiForge site. It needs index.html, page/index.html and a pages/ folder.", root.display()));
        }
        let _ = fs::create_dir_all(root.join("media"));
        let title = fs::read_to_string(root.join("pages").join("pages.json"))
            .ok()
            .and_then(|t| json_unescape_first(&t, "siteTitle"))
            .unwrap_or_else(|| root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Wiki".into()));
        let site = Site { root: root.to_path_buf(), title };
        let _ = site.write_manifest();
        Ok(site)
    }

    /// Creates the folder structure with default templates and a starter article.
    pub fn create(root: &Path, title: &str) -> io::Result<Site> {
        fs::create_dir_all(root.join("media"))?;
        fs::create_dir_all(root.join("page"))?;
        fs::create_dir_all(root.join("pages"))?;
        let site = Site { root: root.to_path_buf(), title: title.to_string() };
        site.write_templates()?;
        site.reset_logo()?;
        Ok(site)
    }

    /// (Re)writes the six template files. Existing files are overwritten.
    pub fn write_templates(&self) -> io::Result<()> {
        fs::write(self.root.join("index.html"), ROOT_HTML)?;
        fs::write(self.root.join("index.css"), ROOT_CSS)?;
        fs::write(self.root.join("index.js"), ROOT_JS)?;
        fs::write(self.root.join("page").join("index.html"), PAGE_HTML)?;
        fs::write(self.root.join("page").join("index.css"), PAGE_CSS)?;
        fs::write(self.root.join("page").join("index.js"), PAGE_JS)?;
        Ok(())
    }

    pub fn pages_dir(&self) -> PathBuf {
        self.root.join("pages")
    }

    pub fn list_pages(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(self.pages_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
                    .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    pub fn page_path(&self, slug: &str) -> PathBuf {
        self.pages_dir().join(format!("{slug}.md"))
    }

    pub fn read_page(&self, slug: &str) -> io::Result<String> {
        fs::read_to_string(self.page_path(slug))
    }

    /// Writes the article and refreshes `pages.json`. Returns the refreshed article list.
    pub fn write_page(&self, slug: &str, md: &str) -> io::Result<Vec<PageInfo>> {
        fs::write(self.page_path(slug), md)?;
        self.refresh()
    }

    pub fn delete_page(&self, slug: &str) -> io::Result<Vec<PageInfo>> {
        fs::remove_file(self.page_path(slug))?;
        self.refresh()
    }

    /// The featured slugs stored in `pages.json`. `None` for sites written before featured
    /// articles existed; those start with every article featured.
    fn read_featured(&self) -> Option<BTreeSet<String>> {
        let text = fs::read_to_string(self.pages_dir().join("pages.json")).ok()?;
        json_string_array(&text, "featured").map(|v| v.into_iter().collect())
    }

    /// The logo path (relative to the site root) stored in `pages.json`. `None` for sites
    /// written before logos existed; their pages show the plain letter badge.
    pub fn read_logo(&self) -> Option<String> {
        let text = fs::read_to_string(self.pages_dir().join("pages.json")).ok()?;
        json_unescape_first(&text, "logo").filter(|l| !l.is_empty())
    }

    /// Removes `media/site-logo.<ext>` for every image extension except `keep`.
    fn remove_logo_files(&self, keep: &str) {
        for ext in IMAGE_EXTS.iter().filter(|e| **e != keep) {
            let _ = fs::remove_file(self.root.join("media").join(format!("site-logo.{ext}")));
        }
    }

    /// Uses the chosen image as the site logo: copies it to `media/site-logo.<ext>` and records it
    /// in `pages.json`. Returns the path relative to the site root.
    pub fn set_logo(&self, src: &Path) -> io::Result<String> {
        let ext = ext_of(&src.to_string_lossy());
        if !IMAGE_EXTS.contains(&ext.as_str()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "the logo must be an image (png, jpg, gif, webp, svg, bmp or avif)"));
        }
        let media = self.root.join("media");
        fs::create_dir_all(&media)?;
        let dest = media.join(format!("site-logo.{ext}"));
        let same = matches!((src.canonicalize(), dest.canonicalize()), (Ok(a), Ok(b)) if a == b);
        if !same {
            fs::copy(src, &dest)?;
        }
        self.remove_logo_files(&ext);
        let rel = format!("media/site-logo.{ext}");
        self.write_json(&self.scan(&self.effective_featured()), Some(&rel))?;
        Ok(rel)
    }

    /// Restores the default WikiForge logo.
    pub fn reset_logo(&self) -> io::Result<String> {
        let media = self.root.join("media");
        fs::create_dir_all(&media)?;
        fs::write(media.join("site-logo.webp"), DEFAULT_LOGO)?;
        self.remove_logo_files("webp");
        let rel = "media/site-logo.webp".to_string();
        self.write_json(&self.scan(&self.effective_featured()), Some(&rel))?;
        Ok(rel)
    }

    /// Reads every article once for its title and cross references.
    fn scan(&self, featured: &BTreeSet<String>) -> Vec<PageInfo> {
        let slugs = self.list_pages();
        let existing: HashSet<&str> = slugs.iter().map(|s| s.as_str()).collect();
        slugs
            .iter()
            .map(|slug| {
                let md = self.read_page(slug).unwrap_or_default();
                let title = extract_title(&md).unwrap_or_else(|| slug.replace('-', " "));
                let links = extract_links(&md).into_iter().filter(|l| l != slug && existing.contains(l.as_str())).collect();
                PageInfo { slug: slug.clone(), title, links, featured: featured.contains(slug) }
            })
            .collect()
    }

    fn effective_featured(&self) -> BTreeSet<String> {
        self.read_featured().unwrap_or_else(|| self.list_pages().into_iter().collect())
    }

    pub fn page_infos(&self) -> Vec<PageInfo> {
        self.scan(&self.effective_featured())
    }

    fn write_json(&self, infos: &[PageInfo], logo: Option<&str>) -> io::Result<()> {
        let featured: Vec<String> = infos.iter().filter(|p| p.featured).map(|p| p.slug.clone()).collect();
        let pages: Vec<String> = infos
            .iter()
            .map(|p| format!("    {{ \"slug\": \"{}\", \"title\": \"{}\", \"links\": {} }}", json_escape(&p.slug), json_escape(&p.title), json_array(&p.links)))
            .collect();
        // `siteTitle`, `logo` and `featured` come before `pages` so that readers looking for the
        // first occurrence of a key can never match an article title.
        let logo_line = logo.map(|l| format!("  \"logo\": \"{}\",\n", json_escape(l))).unwrap_or_default();
        let json = format!(
            "{{\n  \"siteTitle\": \"{}\",\n{}  \"featured\": {},\n  \"pages\": [\n{}\n  ]\n}}\n",
            json_escape(&self.title),
            logo_line,
            json_array(&featured),
            pages.join(",\n")
        );
        fs::write(self.pages_dir().join("pages.json"), json)
    }

    /// Rescans the articles and rewrites `pages.json`.
    pub fn refresh(&self) -> io::Result<Vec<PageInfo>> {
        let infos = self.scan(&self.effective_featured());
        self.write_json(&infos, self.read_logo().as_deref())?;
        Ok(infos)
    }

    pub fn write_manifest(&self) -> io::Result<()> {
        self.refresh().map(|_| ())
    }

    /// Marks the given articles as featured (or not) and rewrites `pages.json`.
    pub fn set_featured(&self, slugs: &[String], on: bool) -> io::Result<Vec<PageInfo>> {
        let mut set = self.effective_featured();
        for s in slugs {
            if on {
                set.insert(s.clone());
            } else {
                set.remove(s);
            }
        }
        let infos = self.scan(&set);
        self.write_json(&infos, self.read_logo().as_deref())?;
        Ok(infos)
    }

    /// Copies a file into `media/` (choosing a free name) and returns the path relative to the site root.
    pub fn import_media(&self, src: &Path) -> io::Result<String> {
        let media = self.root.join("media");
        fs::create_dir_all(&media)?;
        // Already inside media/?
        if let (Ok(a), Ok(b)) = (src.canonicalize(), media.canonicalize()) {
            if let Ok(rel) = a.strip_prefix(&b) {
                return Ok(format!("media/{}", rel.to_string_lossy().replace('\\', "/")));
            }
        }
        let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
        let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let stem: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        let mut name = format!("{stem}{ext}");
        let mut n = 2;
        while media.join(&name).exists() {
            name = format!("{stem}-{n}{ext}");
            n += 1;
        }
        fs::copy(src, media.join(&name))?;
        Ok(format!("media/{name}"))
    }

    /// Writes the showcase article and the sample media it uses.
    pub fn write_showcase(&self) -> io::Result<String> {
        let media = self.root.join("media");
        fs::create_dir_all(&media)?;
        for (name, bytes) in [("sample.svg", SAMPLE_SVG), ("sample.gif", SAMPLE_GIF), ("sample.wav", SAMPLE_WAV), ("sample-states.csv", SAMPLE_CSV)] {
            let p = media.join(name);
            if !p.exists() {
                fs::write(p, bytes)?;
            }
        }
        fs::write(self.page_path("showcase"), SHOWCASE_MD)?;
        self.set_featured(&["showcase".to_string()], true)?;
        Ok("showcase".to_string())
    }

    /// Writes `Example 1` ... `Example 10`, cross-linked, so the site map has something to show.
    /// Existing articles with those names are left alone. Returns how many were created.
    pub fn write_examples(&self) -> io::Result<usize> {
        let mut created = 0;
        for n in 1..=EXAMPLE_LINKS.len() {
            let path = self.page_path(&format!("example-{n}"));
            if path.exists() {
                continue;
            }
            let links: Vec<String> = EXAMPLE_LINKS[n - 1].iter().map(|m| format!("[[Example {m}]]")).collect();
            let md = format!(
                "# Example {n}\n\nThis is example article {n}. It is one of ten linked articles that show how the site map on the home page draws articles as nodes and cross references as edges.\n\nSee also: {}.\n",
                links.join(", ")
            );
            fs::write(path, md)?;
            created += 1;
        }
        self.write_manifest()?;
        Ok(created)
    }

    pub fn media_path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_open_roundtrip() {
        let dir = std::env::temp_dir().join(format!("wikiforge-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let site = Site::create(&dir, "Test \"Wiki\"").unwrap();
        assert!(Site::is_valid(&dir));
        site.write_showcase().unwrap();
        let reopened = Site::open(&dir).unwrap();
        assert_eq!(reopened.title, "Test \"Wiki\"");
        assert_eq!(reopened.list_pages(), vec!["showcase".to_string()]);
        let infos = reopened.page_infos();
        assert_eq!(infos[0].title, "Interactive Article Showcase");
        for f in [
            "index.html",
            "index.css",
            "index.js",
            "page/index.html",
            "page/index.css",
            "page/index.js",
            "pages/pages.json",
            "media/site-logo.webp",
            "media/sample.svg",
            "media/sample.gif",
            "media/sample.wav",
            "media/sample-states.csv",
        ] {
            assert!(dir.join(f).is_file(), "missing {f}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn links_are_extracted() {
        let md = "See [[Main Page]] and [[Other|label]], [[main-page]] again, [x](wiki:Third Page#sec).\n```\n[[ignored]]\n```\n[[ ]]\nNot a link: `[[code]]`, but [[after code]] is.";
        assert_eq!(extract_links(md), vec!["main-page", "other", "third-page", "after-code"]);
    }

    #[test]
    fn json_arrays() {
        let t = "{\"siteTitle\": \"A \\\"featured\\\" wiki\", \"featured\": [\"a\", \"b\\\"c\"], \"pages\": []}";
        assert_eq!(json_string_array(t, "featured").unwrap(), vec!["a".to_string(), "b\"c".to_string()]);
        assert!(json_string_array("{\"pages\": []}", "featured").is_none());
        assert_eq!(json_string_array("{\"featured\": []}", "featured").unwrap().len(), 0);
    }

    #[test]
    fn featured_and_examples() {
        let dir = std::env::temp_dir().join(format!("wikiforge-feat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let site = Site::create(&dir, "Feat").unwrap();
        // a new site has no articles and nothing featured; the showcase is added to the featured list
        assert!(site.page_infos().is_empty());
        assert_eq!(site.write_examples().unwrap(), 10);
        assert_eq!(site.write_examples().unwrap(), 0, "existing examples are not overwritten");
        let infos = site.page_infos();
        assert_eq!(infos.len(), 10);
        assert_eq!(infos.iter().filter(|p| p.featured).count(), 0);
        let ex1 = infos.iter().find(|p| p.slug == "example-1").unwrap();
        assert_eq!(ex1.links, vec!["example-2", "example-3", "example-5"]);
        assert_eq!(ex1.title, "Example 1");

        // every example is reachable from every other one
        let mut seen: HashSet<String> = HashSet::new();
        let mut stack = vec!["example-1".to_string()];
        while let Some(s) = stack.pop() {
            if seen.insert(s.clone()) {
                let info = infos.iter().find(|p| p.slug == s).unwrap();
                stack.extend(info.links.iter().cloned());
            }
        }
        assert_eq!(seen.len(), 10);

        let after = site.set_featured(&["example-3".to_string(), "example-4".to_string()], true).unwrap();
        assert_eq!(after.iter().filter(|p| p.featured).count(), 2);
        site.write_showcase().unwrap();
        let reopened = Site::open(&dir).unwrap();
        let feats: Vec<String> = reopened.page_infos().into_iter().filter(|p| p.featured).map(|p| p.slug).collect();
        assert_eq!(feats, vec!["example-3", "example-4", "showcase"]);
        let json = fs::read_to_string(dir.join("pages/pages.json")).unwrap();
        assert!(json.contains("\"featured\": [\"example-3\", \"example-4\", \"showcase\"]"));
        // deleting a featured article drops it from the list
        let after = reopened.delete_page("example-3").unwrap();
        assert!(after.iter().all(|p| p.slug != "example-3"));
        assert!(!fs::read_to_string(dir.join("pages/pages.json")).unwrap().contains("\"example-3\","));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_manifest_features_everything() {
        let dir = std::env::temp_dir().join(format!("wikiforge-legacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let site = Site::create(&dir, "Old").unwrap();
        site.write_examples().unwrap();
        fs::write(dir.join("pages/pages.json"), "{ \"siteTitle\": \"Old\", \"pages\": [] }").unwrap();
        let infos = Site::open(&dir).unwrap().page_infos();
        assert!(infos.iter().all(|p| p.featured));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn logo_default_custom_and_reset() {
        let dir = std::env::temp_dir().join(format!("wikiforge-logo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let site = Site::create(&dir, "Logo").unwrap();
        assert_eq!(site.read_logo().as_deref(), Some("media/site-logo.webp"));
        assert_eq!(fs::read(dir.join("media/site-logo.webp")).unwrap(), DEFAULT_LOGO);

        let png = dir.join("picked.png");
        fs::write(&png, b"png-bytes").unwrap();
        assert_eq!(site.set_logo(&png).unwrap(), "media/site-logo.png");
        assert_eq!(fs::read(dir.join("media/site-logo.png")).unwrap(), b"png-bytes");
        assert!(!dir.join("media/site-logo.webp").exists(), "the previous logo file is removed");
        let json = fs::read_to_string(dir.join("pages/pages.json")).unwrap();
        assert!(json.contains("\"logo\": \"media/site-logo.png\""), "{json}");
        // the logo survives a rescan and a reopen
        site.refresh().unwrap();
        assert_eq!(Site::open(&dir).unwrap().read_logo().as_deref(), Some("media/site-logo.png"));
        // choosing the file that is already the logo does not truncate it
        site.set_logo(&dir.join("media/site-logo.png")).unwrap();
        assert_eq!(fs::read(dir.join("media/site-logo.png")).unwrap(), b"png-bytes");
        // only images are accepted
        let txt = dir.join("notes.txt");
        fs::write(&txt, b"x").unwrap();
        assert!(site.set_logo(&txt).is_err());

        site.reset_logo().unwrap();
        assert_eq!(site.read_logo().as_deref(), Some("media/site-logo.webp"));
        assert!(!dir.join("media/site-logo.png").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("  Hello, World! "), "hello-world");
        assert_eq!(slugify("My_Page  two"), "my_page-two");
        assert_eq!(slugify("Über Café"), "über-café");
    }
}
