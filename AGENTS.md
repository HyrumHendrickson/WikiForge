# AGENTS.md

Guidance for AI coding agents and human contributors working in this repository.
Read `README.md` first for the product description and the Markdown dialect.

## What this project is

WikiForge is a desktop editor (Rust, egui/eframe) that produces a plain-file static website:
Markdown articles plus a fixed set of HTML/CSS/JS templates. There is no server component
and no database. The generated site is served as-is (for example by GitHub Pages).

## Commands

```
cargo run --release                                      # run the editor
cargo fmt --all --check                                  # formatting (see rustfmt.toml)
cargo clippy --all-targets --locked -- -D warnings       # lints; must be warning-free
cargo test --locked                                      # unit tests and headless UI render tests
node --check assets/index.js && node --check assets/page/index.js   # JS syntax
```

All five must pass before a change is considered done. CI runs the same commands.
The minimum supported Rust version is 1.95 (set by eframe 0.37); do not use newer language features.

## Repository map

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point, window icon |
| `src/app.rs` | Editor UI (menus, page list, block widgets) and UI tests |
| `src/model.rs` | Block model; Markdown parse and serialize (must round-trip) |
| `src/site.rs` | On-disk site layout, `pages.json` manifest, logo, featured list |
| `src/server.rs` | Std-only local preview server (127.0.0.1) |
| `assets/index.*` | Home page template (site map, featured and all articles, search) |
| `assets/page/index.*` | Article viewer template and Markdown renderer (`page/?p=<slug>`) |
| `assets/logo.webp` | Default site logo and editor window icon |
| `assets/showcase.md`, `assets/media/` | Showcase article and its sample media |

## Invariants. Do not break these

1. **Templates are embedded.** Everything under `assets/` is compiled in with `include_str!` /
   `include_bytes!` and written into each site by `Site::write_templates`. Editing a template
   means editing the file in `assets/`, not a generated site.
2. **`slugify` exists twice** (`src/site.rs` and `assets/page/index.js`) and must stay identical.
   Page file names come from `slugify`, which strips path separators; never build a path from
   unsanitized user input.
3. **`pages.json` key order.** `siteTitle`, `logo` and `featured` are written before `pages` so
   the first-occurrence JSON readers in `site.rs` cannot match an article title. Keep that order.
4. **Markdown round-trip.** Every block type must survive `parse_markdown` then `to_markdown`
   unchanged. Add a round-trip test for any new block or embed.
5. **One main page.** The home page (`index.html`, with the site map) is the only main page.
   Do not reintroduce a starter article or a Navigation block in the sidebar; the sidebar lists
   featured articles only.
6. **Backward compatibility.** Sites created by older versions must still open: a manifest with
   no `featured` key features every article; a manifest with no `logo` key shows the letter badge.
7. **Escape by default.** Article text reaches the DOM only through `esc()` or an explicit
   allow-listed builder in `page/index.js`. Raw HTML in Markdown is escaped, not passed through.
8. **Minimal dependencies.** The preview server is std-only on purpose. Do not add a crate or a
   JS library without a stated reason in the PR; third-party scripts load from the hosts listed in
   `CONFIG` at the top of `assets/page/index.js` and nowhere else.

## Conventions

* Rust 2021, formatted with the checked-in `rustfmt.toml` (width 160). No `unwrap()` on I/O or
  user input in non-test code; surface errors in the status bar (`self.status`).
* Comments explain why, not what. Keep doc comments on public items.
* JavaScript is plain browser JS (no build step, no modules, no npm).
* Commit messages: imperative mood, subject under 72 characters; use a body to explain why.
* Update `CHANGELOG.md` (Unreleased section) for any user-visible change, and `README.md` when
  behavior or the site layout changes.

## Security rules for agents

* Never commit secrets, API keys or personal data. The Desmos key in `CONFIG` is Desmos's public demo key.
* Treat site folders, article Markdown and imported media as untrusted input.
* Report suspected vulnerabilities privately, following `SECURITY.md`; do not describe an
  unpatched vulnerability in a public issue, commit message or PR.
* Do not weaken the preview server's checks (loopback bind, GET/HEAD only, rejection of `..`
  and absolute path components) or add outbound network calls from the editor.

## Definition of done

Tests added or updated, the five commands above pass, docs and changelog updated, and the diff
contains no unrelated reformatting.
