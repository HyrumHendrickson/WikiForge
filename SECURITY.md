# Security Policy

## Supported versions

WikiForge is pre-1.0. Security fixes are made on the `main` branch and released from it;
older versions are not patched.

## Reporting a vulnerability

Please report vulnerabilities privately. Do not open a public issue.

Use GitHub's private reporting: **Security > Report a vulnerability** on
<https://github.com/HyrumHendrickson/WikiForge>
(direct link: <https://github.com/HyrumHendrickson/WikiForge/security/advisories/new>).

Include the affected version or commit, a description of the impact, and reproduction steps or a
proof of concept. You can expect an acknowledgement within 7 days and a fix or mitigation plan
within 30 days. Reporters are credited in the advisory unless they ask otherwise.

## Scope

In scope: the editor application, the local preview server, and the site templates under
`assets/` that are written into every generated wiki. Out of scope: vulnerabilities in third-party
services a wiki can embed (Rust Playground, Compiler Explorer, Desmos, OpenStreetMap, YouTube) and
in content authored by the owner of a wiki.

## Security model and known characteristics

These describe current behavior so that site authors can make informed decisions.

**Preview server (`src/server.rs`).** Started by *Preview in browser*. It binds to `127.0.0.1` on a
OS-assigned port, answers `GET` and `HEAD` only, rejects `..` and absolute path components, and
serves only the opened site folder. It follows symbolic links inside that folder and does not
validate the `Host` header. Do not open site folders you do not trust.

**Article rendering (`assets/page/index.js`).** Article text is HTML-escaped; raw HTML and
`<script>` tags in Markdown are shown as text. Page file names derive from `slugify`, which strips
path separators.

**Runnable code blocks.** Readers can run code in articles.
* JavaScript runs in the page's own origin when the reader runs it. Review contributions to a
  published wiki the same way you would review code.
* Python runs in the reader's browser through Pyodide.
* Rust source is sent to `play.rust-lang.org`; C, C++ and Java source is sent to `godbolt.org`.
  Code, and anything a reader types into it, leaves their machine. Do not put secrets in runnable blocks.

**Third-party scripts.** Generated pages load KaTeX, CodeMirror, Leaflet, Pyodide, model-viewer and
Desmos from cdnjs, jsDelivr, Google Hosted Libraries and desmos.com at pinned versions, without
Subresource Integrity hashes. Sites with strict requirements should vendor these files or add SRI
hashes and a Content-Security-Policy. The Desmos key shipped in `CONFIG` is Desmos's public demo
key and should be replaced for production use.

**Editor.** The editor makes no network requests of its own apart from opening the preview URL in a
browser. It reads and writes only inside the chosen site folder, plus files the user explicitly picks.

## Dependencies

Dependabot proposes Cargo and GitHub Actions updates weekly, and `cargo audit` runs in CI on
every change to `Cargo.lock` and on a weekly schedule.
