//! The on-disk site layout.
//!
//! ```text
//! root/
//!   index.html  index.css  index.js        home page
//!   media/                                 images, videos, 3D models
//!   page/index.html index.css index.js     article viewer
//!   pages/*.md                             one Markdown file per article
//!   pages/pages.json                       list of articles (written by the editor)
//! ```

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
const MAIN_PAGE_MD: &str = include_str!("../assets/main-page.md");
const SAMPLE_SVG: &[u8] = include_bytes!("../assets/media/sample.svg");
const SAMPLE_GIF: &[u8] = include_bytes!("../assets/media/sample.gif");

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "webm", "ogv", "mov", "m4v"];
pub const MODEL_EXTS: &[&str] = &["glb", "gltf"];

#[derive(Clone)]
pub struct Site {
    pub root: PathBuf,
    pub title: String,
}

#[derive(Clone)]
pub struct PageInfo {
    pub slug: String,
    pub title: String,
}

pub fn ext_of(path: &str) -> String {
    Path::new(path.split('?').next().unwrap_or(path))
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Mirrors `slugify` in page/index.js so that `[[Page Name]]` links resolve to the right file.
pub fn slugify(name: &str) -> String {
    let kept: String = name
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '_' || *c == '-')
        .collect();
    let s = kept.split_whitespace().collect::<Vec<_>>().join("-");
    if s.is_empty() { "section".to_string() } else { s }
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

fn json_unescape_first(text: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = text.find(&pat)? + pat.len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
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

impl Site {
    /// A folder is a valid site when it has the three required entries.
    pub fn is_valid(root: &Path) -> bool {
        root.join("index.html").is_file() && root.join("page").join("index.html").is_file() && root.join("pages").is_dir()
    }

    pub fn open(root: &Path) -> Result<Site, String> {
        if !Self::is_valid(root) {
            return Err(format!(
                "{} is not a WikiForge site. It needs index.html, page/index.html and a pages/ folder.",
                root.display()
            ));
        }
        let _ = fs::create_dir_all(root.join("media"));
        let title = fs::read_to_string(root.join("pages").join("pages.json"))
            .ok()
            .and_then(|t| json_unescape_first(&t, "siteTitle"))
            .unwrap_or_else(|| {
                root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Wiki".into())
            });
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
        if site.list_pages().is_empty() {
            fs::write(root.join("pages").join("main-page.md"), MAIN_PAGE_MD)?;
        }
        site.write_manifest()?;
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

    pub fn write_page(&self, slug: &str, md: &str) -> io::Result<()> {
        fs::write(self.page_path(slug), md)?;
        self.write_manifest()
    }

    pub fn delete_page(&self, slug: &str) -> io::Result<()> {
        fs::remove_file(self.page_path(slug))?;
        self.write_manifest()
    }

    pub fn page_infos(&self) -> Vec<PageInfo> {
        self.list_pages()
            .into_iter()
            .map(|slug| {
                let title = self
                    .read_page(&slug)
                    .ok()
                    .and_then(|t| extract_title(&t))
                    .unwrap_or_else(|| slug.replace('-', " "));
                PageInfo { slug, title }
            })
            .collect()
    }

    pub fn write_manifest(&self) -> io::Result<()> {
        let pages: Vec<String> = self
            .page_infos()
            .iter()
            .map(|p| format!("    {{ \"slug\": \"{}\", \"title\": \"{}\" }}", json_escape(&p.slug), json_escape(&p.title)))
            .collect();
        let json = format!(
            "{{\n  \"siteTitle\": \"{}\",\n  \"pages\": [\n{}\n  ]\n}}\n",
            json_escape(&self.title),
            pages.join(",\n")
        );
        fs::write(self.pages_dir().join("pages.json"), json)
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
        let stem: String = stem
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
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
        for (name, bytes) in [("sample.svg", SAMPLE_SVG), ("sample.gif", SAMPLE_GIF)] {
            let p = media.join(name);
            if !p.exists() {
                fs::write(p, bytes)?;
            }
        }
        fs::write(self.page_path("showcase"), SHOWCASE_MD)?;
        self.write_manifest()?;
        Ok("showcase".to_string())
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
        assert_eq!(reopened.list_pages(), vec!["main-page".to_string(), "showcase".to_string()]);
        let infos = reopened.page_infos();
        assert_eq!(infos[1].title, "Interactive Article Showcase");
        for f in ["index.html", "index.css", "index.js", "page/index.html", "page/index.css", "page/index.js", "pages/pages.json", "media/sample.svg", "media/sample.gif"] {
            assert!(dir.join(f).is_file(), "missing {f}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("  Hello, World! "), "hello-world");
        assert_eq!(slugify("My_Page  two"), "my_page-two");
        assert_eq!(slugify("Über Café"), "über-café");
    }
}
