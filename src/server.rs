//! A minimal static file server (std only) so that pages, which load their Markdown with
//! `fetch()`, can be previewed in a browser straight from the editor.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

pub type SharedRoot = Arc<Mutex<PathBuf>>;

pub struct PreviewServer {
    pub port: u16,
    pub root: SharedRoot,
}

impl PreviewServer {
    pub fn start(root: PathBuf) -> std::io::Result<PreviewServer> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let shared: SharedRoot = Arc::new(Mutex::new(root));
        let for_thread = shared.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let root = for_thread.clone();
                thread::spawn(move || {
                    let _ = handle(stream, &root);
                });
            }
        });
        Ok(PreviewServer { port, root: shared })
    }

    pub fn set_root(&self, root: PathBuf) {
        if let Ok(mut r) = self.root.lock() {
            *r = root;
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, path)
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("md") => "text/markdown; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("ogv") => "video/ogg",
        Some("mov") => "video/quicktime",
        Some("glb") => "model/gltf-binary",
        Some("gltf") => "model/gltf+json",
        Some("csv") => "text/csv",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn respond(stream: &mut TcpStream, status: &str, headers: &[(&str, String)], body: &[u8]) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status}\r\nConnection: close\r\nCache-Control: no-store\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)
}

fn handle(mut stream: TcpStream, root: &SharedRoot) -> std::io::Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let first = req.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, "405 Method Not Allowed", &[], b"");
    }
    let path_part = target.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode(path_part);

    let root = root.lock().map(|r| r.clone()).unwrap_or_default();
    let mut file = root.clone();
    for comp in Path::new(decoded.trim_start_matches('/')).components() {
        match comp {
            Component::Normal(c) => file.push(c),
            Component::CurDir => {}
            _ => return respond(&mut stream, "403 Forbidden", &[], b"forbidden"),
        }
    }
    if file.is_dir() {
        if !path_part.ends_with('/') {
            return respond(&mut stream, "301 Moved Permanently", &[("Location", format!("{path_part}/"))], b"");
        }
        file.push("index.html");
    }
    let Ok(mut f) = fs::File::open(&file) else {
        return respond(&mut stream, "404 Not Found", &[("Content-Type", "text/plain".into())], b"404 not found");
    };
    let len = f.metadata()?.len();
    let ct = content_type(&file).to_string();

    // Byte ranges are needed for seeking in <video>.
    let range = req.lines().find_map(|l| {
        let l = l.to_lowercase();
        l.strip_prefix("range: bytes=").map(|r| r.trim().to_string())
    });
    if let Some(r) = range {
        let (a, b) = r.split_once('-').unwrap_or(("0", ""));
        let start: u64 = a.parse().unwrap_or(0).min(len.saturating_sub(1));
        let end: u64 = b.parse().unwrap_or(len.saturating_sub(1)).min(len.saturating_sub(1));
        if start <= end && len > 0 {
            f.seek(SeekFrom::Start(start))?;
            let mut data = vec![0u8; (end - start + 1) as usize];
            f.read_exact(&mut data)?;
            return respond(
                &mut stream,
                "206 Partial Content",
                &[("Content-Type", ct), ("Content-Range", format!("bytes {start}-{end}/{len}")), ("Content-Length", data.len().to_string()), ("Accept-Ranges", "bytes".into())],
                if method == "HEAD" { b"" } else { &data },
            );
        }
    }
    let mut data = Vec::with_capacity(len as usize);
    f.read_to_end(&mut data)?;
    respond(
        &mut stream,
        "200 OK",
        &[("Content-Type", ct), ("Content-Length", data.len().to_string()), ("Accept-Ranges", "bytes".into())],
        if method == "HEAD" { b"" } else { &data },
    )
}
