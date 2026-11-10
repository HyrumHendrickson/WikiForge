# WikiForge

A desktop editor (Rust, egui) for building interactive, Wikipedia-style websites.
Sites are plain files, so you can open the folder, `git init`, and push it to GitHub
(GitHub Pages serves it as-is).

## Build and run

```
cargo run --release
```

Requires Rust 1.95 or newer (eframe 0.37). On Linux you also need the usual windowing
libraries (for Debian/Ubuntu: `libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev`)
and an XDG desktop portal for the file dialogs.

## Site layout

```
my-wiki/
  index.html  index.css  index.js      home page (lists articles, search)
  media/                               images, GIFs, videos, .glb models
  page/
    index.html  index.css  index.js    article viewer: page/?p=<slug>
  pages/
    main-page.md                       one Markdown file per article
    showcase.md
    pages.json                         article list, rewritten by the editor on save
```

`page/index.js` fetches `../pages/<slug>.md`, renders it, and activates the widgets.
Because it uses `fetch()`, open sites over HTTP (the editor's **Preview in browser**
button starts a small local server; `python3 -m http.server` also works). Opening
`index.html` straight from disk will not load articles.

**File > Open site** accepts any folder that has `index.html`, `page/index.html`
and a `pages/` directory.

## Editor

* **File**: new site, open site, save (Ctrl+S). Auto-save runs 3 seconds after you stop typing.
* **Page > Generate showcase page**: writes `pages/showcase.md` with one example of every embed type.
* Each article is a vertical list of blocks. Drag the "Drag" handle (or use Up/Down) to reorder.
* Text blocks: click to type; choose font, size, color and alignment; wrap the selection in
  bold, italic, code, page link (blue), web link (green), inline math, or add a footnote.
* Image / video / 3D model blocks copy the chosen file into `media/`.
* Code blocks have an **Include canvas** checkbox.

## Markdown dialect

Standard Markdown (headings, lists, quotes, tables, emphasis, code spans) plus:

| Syntax | Result |
|---|---|
| `[[Page name]]`, `[[page-name\|label]]` | internal link (blue) |
| `[label](https://...)` | external link (green) |
| `text[^1]` and `[^1]: note` | footnote, collected under "Notes" |
| `$x^2$`, `$$ ... $$` | LaTeX (KaTeX) |
| ```` ```python ````, `c`, `cpp`, `rust`, `javascript`, `java` | runnable code window; add `canvas` after the language for a drawing canvas |
| ```` ```sheet ```` + CSV | interactive spreadsheet with formulas (`=SUM(A1:A3)`; separate arguments with `;`) |
| ```` ```desmos ```` | Desmos graph, one expression per line |
| ```` ```map ```` | Leaflet map: `center:`, `zoom:`, `marker: lat, lon \| label` |
| ```` ```model src="media/x.glb" ```` | interactive 3D model |
| `::: figure src=media/x.png caption="..." align=right width=300` | image / GIF / video |
| `::: infobox`, `::: note`, `::: tip`, `::: warning` | infobox and callouts |
| `::: text font=Georgia size=20 color=#7a1f1f align=center` | styled text block |

Plain Markdown tables are static; `sheet` blocks are interactive.

## Canvas

JavaScript and Python call the drawing functions directly. C, C++, Rust and Java print
commands, one per line, starting with `@`:

```
@rect x y w h [color] [fill|stroke]      @ellipse cx cy rx ry [color]     @circle cx cy r [color]
@line x1 y1 x2 y2 [color]                @text x y words...               @pixel x y [color]
@forward n  @back n  @left a  @right a   @penup  @pendown  @goto x y     @home  @setheading a
@color c  @pensize w  @dot r  @arc r [extent]  @hideturtle  @showturtle  @clear [color]  @size w h
```

Pixel commands use a top-left origin; turtle commands use a centre origin with y pointing up.
Colors must not contain spaces.

## Where code runs

* **JavaScript, Python**: in the reader's browser (Python through Pyodide).
* **Rust**: sent to the Rust Playground (`play.rust-lang.org`).
* **C, C++, Java**: sent to Compiler Explorer (`godbolt.org`), which picks the newest GCC / JDK.

The remote runners are third-party services with their own limits and availability; endpoints
live in the `CONFIG` block at the top of `page/index.js`. The Desmos key in that block is
Desmos's public demo key; replace it with your own for production use.

## Tests

```
cargo test
```

Covers Markdown round-tripping of every block type, the site layout, and a headless render
of the editor UI.
