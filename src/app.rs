//! The editor window.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};

use crate::model::*;
use crate::server::PreviewServer;
use crate::site::*;

const FONT_CHOICES: &[&str] = &[
    "(default)", "serif", "sans", "mono", "Georgia", "Times New Roman", "Palatino", "Arial", "Verdana", "Trebuchet MS", "Courier New",
];
const CODE_LANGS: &[(&str, &str)] = &[
    ("python", "Python"),
    ("c", "C"),
    ("cpp", "C++"),
    ("rust", "Rust"),
    ("javascript", "JavaScript"),
    ("java", "Java"),
    ("text", "Plain (not runnable)"),
];
const ADD_KINDS: &[Kind] = &[
    Kind::Heading,
    Kind::Text,
    Kind::Figure,
    Kind::Math,
    Kind::Table,
    Kind::Sheet,
    Kind::Code,
    Kind::Desmos,
    Kind::Map,
    Kind::Model,
    Kind::Timeline,
    Kind::Audio,
    Kind::CsvSearch,
    Kind::Infobox,
    Kind::Callout,
    Kind::Footnote,
    Kind::Raw,
];

enum Action {
    Up(usize),
    Down(usize),
    Delete(usize),
    Duplicate(usize),
    Insert(usize, Kind),
    Move(usize, usize),
    FootnoteRef(usize),
}

pub struct App {
    site: Option<Site>,
    pages: Vec<PageInfo>,
    current: Option<String>,
    blocks: Vec<Block>,
    saved_md: String,
    status: String,
    new_page_name: String,
    server: Option<PreviewServer>,
    confirm_delete: bool,
    autosave: bool,
    last_change: Instant,
    /// Filter for the page list in the sidebar (the site may have thousands of articles).
    page_filter: String,
    /// "Featured articles" window.
    show_featured: bool,
    featured_filter: String,
    featured_only: bool,
}

fn text_id(block_id: u64) -> egui::Id {
    egui::Id::unique(("block-text", block_id))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        App {
            site: None,
            pages: Vec::new(),
            current: None,
            blocks: Vec::new(),
            saved_md: String::new(),
            status: "Create a new site or open an existing one from the File menu.".into(),
            new_page_name: String::new(),
            server: None,
            confirm_delete: false,
            autosave: true,
            last_change: Instant::now(),
            page_filter: String::new(),
            show_featured: false,
            featured_filter: String::new(),
            featured_only: false,
        }
    }

    // ------------------------------------------------------------ file operations

    fn is_dirty(&self) -> bool {
        self.current.is_some() && to_markdown(&self.blocks) != self.saved_md
    }

    fn save(&mut self) {
        let (Some(site), Some(slug)) = (self.site.clone(), self.current.clone()) else { return };
        let md = to_markdown(&self.blocks);
        match site.write_page(&slug, &md) {
            Ok(infos) => {
                self.saved_md = md;
                self.pages = infos;
                self.status = format!("Saved pages/{slug}.md");
            }
            Err(e) => self.status = format!("Could not save: {e}"),
        }
    }

    fn load_site(&mut self, site: Site) {
        if self.is_dirty() {
            self.save();
        }
        self.pages = site.page_infos();
        if let Some(s) = &self.server {
            s.set_root(site.root.clone());
        }
        let first = self.pages.first().map(|p| p.slug.clone());
        self.status = format!("Opened {}", site.root.display());
        self.site = Some(site);
        self.current = None;
        self.blocks.clear();
        if let Some(slug) = first {
            self.select_page(&slug);
        }
    }

    fn new_site(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Choose a folder for the new site").pick_folder() else { return };
        let title = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "My Wiki".into());
        match Site::create(&dir, &title) {
            Ok(site) => self.load_site(site),
            Err(e) => self.status = format!("Could not create the site: {e}"),
        }
    }

    fn open_site(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Open a WikiForge site folder").pick_folder() else { return };
        match Site::open(&dir) {
            Ok(site) => self.load_site(site),
            Err(e) => self.status = e,
        }
    }

    fn select_page(&mut self, slug: &str) {
        if self.is_dirty() {
            self.save();
        }
        let Some(site) = self.site.clone() else { return };
        match site.read_page(slug) {
            Ok(md) => {
                self.blocks = parse_markdown(&md);
                self.saved_md = to_markdown(&self.blocks);
                self.current = Some(slug.to_string());
                self.status = format!("Editing pages/{slug}.md");
            }
            Err(e) => self.status = format!("Could not read {slug}.md: {e}"),
        }
    }

    #[cfg(test)]
    fn create_page_named(&mut self, name: &str) {
        self.new_page_name = name.to_string();
        self.create_page();
    }

    fn create_page(&mut self) {
        let name = self.new_page_name.trim().to_string();
        let slug = slugify(&name);
        let Some(site) = self.site.clone() else { return };
        if name.is_empty() {
            return;
        }
        if site.page_path(&slug).exists() {
            self.status = format!("A page named {slug} already exists; opened it.");
        } else {
            let md = format!("# {name}\n\nThis article is about **{name}**.\n");
            match site.write_page(&slug, &md) {
                Ok(infos) => self.pages = infos,
                Err(e) => {
                    self.status = format!("Could not create page: {e}");
                    return;
                }
            }
            self.status = format!("Created pages/{slug}.md");
        }
        self.new_page_name.clear();
        self.select_page(&slug);
    }

    fn delete_current(&mut self) {
        let (Some(site), Some(slug)) = (self.site.clone(), self.current.clone()) else { return };
        match site.delete_page(&slug) {
            Ok(infos) => {
                self.pages = infos;
                self.current = None;
                self.blocks.clear();
                self.saved_md.clear();
                self.status = format!("Deleted pages/{slug}.md");
                if let Some(p) = self.pages.first().map(|p| p.slug.clone()) {
                    self.select_page(&p);
                }
            }
            Err(e) => self.status = format!("Could not delete: {e}"),
        }
    }

    fn generate_showcase(&mut self) {
        let Some(site) = self.site.clone() else { return };
        if self.is_dirty() {
            self.save();
        }
        match site.write_showcase() {
            Ok(slug) => {
                self.pages = site.page_infos();
                self.status = "Generated the showcase page (pages/showcase.md) and sample media.".into();
                self.current = None;
                self.select_page(&slug);
            }
            Err(e) => self.status = format!("Could not write the showcase page: {e}"),
        }
    }

    fn add_examples(&mut self) {
        let Some(site) = self.site.clone() else { return };
        if self.is_dirty() {
            self.save();
        }
        match site.write_examples() {
            Ok(0) => self.status = "Example 1 to Example 10 already exist; nothing was added.".into(),
            Ok(n) => {
                self.pages = site.page_infos();
                self.status = format!("Added {n} linked example articles (Example 1 ... Example 10). Open the home page to see them on the site map.");
            }
            Err(e) => self.status = format!("Could not add the example articles: {e}"),
        }
    }

    fn set_featured(&mut self, slugs: &[String], on: bool) {
        let Some(site) = self.site.clone() else { return };
        if slugs.is_empty() {
            return;
        }
        match site.set_featured(slugs, on) {
            Ok(infos) => {
                self.pages = infos;
                let n = self.pages.iter().filter(|p| p.featured).count();
                self.status = format!("{n} featured article{} (shown in the sidebar).", if n == 1 { "" } else { "s" });
            }
            Err(e) => self.status = format!("Could not update the featured list: {e}"),
        }
    }

    fn set_logo(&mut self) {
        let Some(site) = self.site.clone() else { return };
        let Some(path) = rfd::FileDialog::new().set_title("Choose a logo image").add_filter("Images", IMAGE_EXTS).pick_file() else { return };
        self.status = match site.set_logo(&path) {
            Ok(rel) => format!("Logo set ({rel})."),
            Err(e) => format!("Could not set the logo: {e}"),
        };
    }

    fn reset_logo(&mut self) {
        let Some(site) = self.site.clone() else { return };
        self.status = match site.reset_logo() {
            Ok(_) => "Logo reset to the WikiForge default.".into(),
            Err(e) => format!("Could not reset the logo: {e}"),
        };
    }

    fn preview(&mut self) {
        let (Some(site), Some(slug)) = (self.site.clone(), self.current.clone()) else { return };
        self.save();
        if self.server.is_none() {
            match PreviewServer::start(site.root.clone()) {
                Ok(s) => self.server = Some(s),
                Err(e) => {
                    self.status = format!("Could not start the preview server: {e}");
                    return;
                }
            }
        }
        if let Some(s) = &self.server {
            s.set_root(site.root.clone());
            let url = s.url(&format!("page/?p={slug}"));
            match webbrowser::open(&url) {
                Ok(()) => self.status = format!("Preview: {url}"),
                Err(e) => self.status = format!("Could not open a browser ({e}). Visit {url}"),
            }
        }
    }

    fn add_media(&mut self) {
        let Some(site) = self.site.clone() else { return };
        let mut exts: Vec<&str> = Vec::new();
        exts.extend(IMAGE_EXTS);
        exts.extend(VIDEO_EXTS);
        exts.extend(MODEL_EXTS);
        exts.extend(AUDIO_EXTS);
        let Some(path) = rfd::FileDialog::new().add_filter("Images, videos, audio and 3D models", &exts).pick_file() else { return };
        match site.import_media(&path) {
            Ok(rel) => {
                let ext = ext_of(&rel);
                let mut b;
                if MODEL_EXTS.contains(&ext.as_str()) {
                    b = Block::new(Kind::Model);
                    b.set_attr("src", rel.clone());
                } else if AUDIO_EXTS.contains(&ext.as_str()) {
                    b = Block::new(Kind::Audio);
                    b.set_attr("src", rel.clone());
                    let stem = std::path::Path::new(&rel).file_stem().map(|s| s.to_string_lossy().replace(['-', '_'], " ")).unwrap_or_default();
                    b.set_attr("title", stem);
                } else {
                    b = Block::new(Kind::Figure);
                    b.set_attr("src", rel.clone());
                    if VIDEO_EXTS.contains(&ext.as_str()) {
                        b.set_attr("align", "center");
                        b.set_attr("width", "480");
                    }
                }
                self.blocks.push(b);
                self.status = format!("Added {rel}");
            }
            Err(e) => self.status = format!("Could not copy the file: {e}"),
        }
    }

    // ------------------------------------------------------------ UI pieces

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New site...").clicked() {
                    ui.close();
                    self.new_site();
                }
                if ui.button("Open site...").clicked() {
                    ui.close();
                    self.open_site();
                }
                ui.separator();
                if ui.add_enabled(self.current.is_some(), egui::Button::new("Save   Ctrl+S")).clicked() {
                    ui.close();
                    self.save();
                }
                ui.checkbox(&mut self.autosave, "Auto-save after 3 seconds idle");
            });
            ui.menu_button("Page", |ui| {
                let has_site = self.site.is_some();
                if ui.add_enabled(has_site, egui::Button::new("Generate showcase page")).clicked() {
                    ui.close();
                    self.generate_showcase();
                }
                if ui.add_enabled(has_site, egui::Button::new("Add 10 example articles")).on_hover_text("Example 1 ... Example 10, linked to each other, to try out the site map").clicked() {
                    ui.close();
                    self.add_examples();
                }
                if ui.add_enabled(self.current.is_some(), egui::Button::new("Preview in browser")).clicked() {
                    ui.close();
                    self.preview();
                }
                ui.separator();
                if ui.add_enabled(self.current.is_some(), egui::Button::new("Delete this page...")).clicked() {
                    ui.close();
                    self.confirm_delete = true;
                }
            });
            ui.menu_button("Site", |ui| {
                if ui.add_enabled(self.site.is_some(), egui::Button::new("Featured articles...")).on_hover_text("Choose which articles are listed in the sidebar").clicked() {
                    ui.close();
                    self.show_featured = true;
                }
                if ui.add_enabled(self.site.is_some(), egui::Button::new("Set logo...")).on_hover_text("Choose an image to use as this wiki's logo and icon").clicked() {
                    ui.close();
                    self.set_logo();
                }
                if ui.add_enabled(self.site.is_some(), egui::Button::new("Reset logo to default")).clicked() {
                    ui.close();
                    self.reset_logo();
                }
                if ui.add_enabled(self.site.is_some(), egui::Button::new("Restore default templates")).on_hover_text("Overwrites index.html/css/js and page/index.html/css/js").clicked() {
                    ui.close();
                    if let Some(site) = &self.site {
                        self.status = match site.write_templates() {
                            Ok(()) => "Default templates restored.".into(),
                            Err(e) => format!("Could not write templates: {e}"),
                        };
                    }
                }
                if ui.add_enabled(self.site.is_some(), egui::Button::new("Open site folder")).clicked() {
                    ui.close();
                    if let Some(site) = &self.site {
                        let _ = webbrowser::open(&site.root.to_string_lossy());
                    }
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let Some(site) = self.site.clone() else { return };
        ui.add_space(4.0);
        ui.label(RichText::new("Site title").small());
        let mut title = site.title.clone();
        if ui.text_edit_singleline(&mut title).changed() {
            if let Some(s) = self.site.as_mut() {
                s.title = title;
                let _ = s.write_manifest();
            }
        }
        ui.label(RichText::new(site.root.display().to_string()).small().weak());
        ui.separator();
        ui.heading("Pages");
        let featured_count = self.pages.iter().filter(|p| p.featured).count();
        ui.label(RichText::new(format!("{} articles, {} featured (*)", self.pages.len(), featured_count)).small().weak());
        ui.add(egui::TextEdit::singleline(&mut self.page_filter).hint_text("Filter pages").desired_width(f32::INFINITY));
        let q = self.page_filter.trim().to_lowercase();
        let visible: Vec<usize> = (0..self.pages.len())
            .filter(|&i| q.is_empty() || self.pages[i].title.to_lowercase().contains(&q) || self.pages[i].slug.contains(&q))
            .collect();
        let mut to_select: Option<String> = None;
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
        // show_rows only lays out the visible rows, so thousands of articles stay fast
        egui::ScrollArea::vertical().id_salt("page-list").max_height((ui.available_height() - 130.0).max(80.0)).show_rows(ui, row_h, visible.len(), |ui, range| {
            for row in range {
                let p = &self.pages[visible[row]];
                let selected = self.current.as_deref() == Some(p.slug.as_str());
                let text = if p.featured { format!("* {}", p.title) } else { p.title.clone() };
                if ui.selectable_label(selected, text).on_hover_text(format!("pages/{}.md", p.slug)).clicked() {
                    to_select = Some(p.slug.clone());
                }
            }
        });
        if let Some(s) = to_select {
            self.select_page(&s);
        }
        ui.separator();
        ui.label("New page");
        let resp = ui.add(egui::TextEdit::singleline(&mut self.new_page_name).hint_text("Page title"));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Create page").clicked() || enter {
            self.create_page();
        }
        if ui.button("Add 10 example articles").on_hover_text("Example 1 ... Example 10, linked to each other, to try out the site map").clicked() {
            self.add_examples();
        }
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.heading(RichText::new("WikiForge").size(34.0));
            ui.label("Build interactive, Wikipedia-style sites as plain files you can commit to GitHub.");
            ui.add_space(24.0);
            if ui.add(egui::Button::new(RichText::new("New site...").size(18.0)).min_size(egui::vec2(220.0, 40.0))).clicked() {
                self.new_site();
            }
            ui.add_space(8.0);
            if ui.add(egui::Button::new(RichText::new("Open site...").size(18.0)).min_size(egui::vec2(220.0, 40.0))).clicked() {
                self.open_site();
            }
            ui.add_space(20.0);
            ui.label(RichText::new("A site folder contains index.html/css/js, media/, page/ and pages/.").weak());
        });
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let Some(site) = self.site.clone() else { return };
        let Some(slug) = self.current.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label("Select a page on the left, or create a new one.");
            });
            return;
        };
        let dirty = self.is_dirty();

        ui.horizontal_wrapped(|ui| {
            ui.heading(format!("pages/{slug}.md"));
            if dirty {
                ui.label(RichText::new("unsaved changes").color(Color32::from_rgb(190, 110, 0)));
            }
            let mut featured = self.pages.iter().find(|p| p.slug == slug).map(|p| p.featured).unwrap_or(false);
            if ui.checkbox(&mut featured, "Featured").on_hover_text("Featured articles are listed in the sidebar. Others are found through search and the site map.").changed() {
                self.set_featured(&[slug.clone()], featured);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Preview in browser").clicked() {
                    self.preview();
                }
                if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
                    self.save();
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("Add block", |ui| {
                for k in ADD_KINDS {
                    if ui.button(k.label()).clicked() {
                        ui.close();
                        self.blocks.push(Block::new(*k));
                    }
                }
            });
            if ui.button("Add image / video / audio / 3D model...").clicked() {
                self.add_media();
            }
            ui.label(RichText::new(format!("{} blocks", self.blocks.len())).weak());
        });
        ui.separator();

        let mut action: Option<Action> = None;
        let mut status = String::new();
        let count = self.blocks.len();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for idx in 0..count {
                let frame = egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(8));
                let (_, dropped) = ui.dnd_drop_zone::<usize, ()>(frame, |ui| {
                    draw_block(ui, &mut self.blocks[idx], idx, count, &site, &mut action, &mut status);
                });
                if let Some(from) = dropped {
                    if *from != idx {
                        action = Some(Action::Move(*from, idx));
                    }
                }
                ui.add_space(6.0);
            }
            ui.add_space(24.0);
        });
        if !status.is_empty() {
            self.status = status;
        }
        if let Some(a) = action {
            self.apply(ui.ctx(), a);
        }
    }

    fn apply(&mut self, ctx: &egui::Context, a: Action) {
        let n = self.blocks.len();
        match a {
            Action::Up(i) if i > 0 && i < n => self.blocks.swap(i, i - 1),
            Action::Down(i) if i + 1 < n => self.blocks.swap(i, i + 1),
            Action::Delete(i) if i < n => {
                self.blocks.remove(i);
            }
            Action::Duplicate(i) if i < n => {
                let d = self.blocks[i].duplicate();
                self.blocks.insert(i + 1, d);
            }
            Action::Insert(i, kind) => {
                let at = i.min(n);
                self.blocks.insert(at, Block::new(kind));
            }
            Action::Move(from, to) if from < n && to < n => {
                let b = self.blocks.remove(from);
                self.blocks.insert(to, b);
            }
            Action::FootnoteRef(i) if i < n => {
                let id = next_footnote_id(&self.blocks);
                let tid = text_id(self.blocks[i].id);
                wrap_selection(ctx, tid, &mut self.blocks[i].text, &format!("[^{id}]"), "", "");
                let mut f = Block::new(Kind::Footnote);
                f.set_attr("id", id);
                self.blocks.push(f);
                self.status = "Footnote added at the end of the page; edit its text there.".into();
            }
            _ => {}
        }
    }

    fn featured_window(&mut self, ctx: &egui::Context) {
        if !self.show_featured {
            return;
        }
        let mut open = true;
        let mut changes: Vec<(String, bool)> = Vec::new();
        let mut bulk: Option<(Vec<String>, bool)> = None;
        egui::Window::new("Featured articles").open(&mut open).default_size([440.0, 520.0]).show(ctx, |ui| {
            ui.label("Featured articles are listed in the sidebar of the home page and of every article. Every article stays reachable through search, the home page list and the site map.");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.featured_filter).hint_text("Filter by title"));
                ui.checkbox(&mut self.featured_only, "Featured only");
            });
            let q = self.featured_filter.trim().to_lowercase();
            let visible: Vec<usize> = (0..self.pages.len())
                .filter(|&i| {
                    let p = &self.pages[i];
                    (!self.featured_only || p.featured) && (q.is_empty() || p.title.to_lowercase().contains(&q) || p.slug.contains(&q))
                })
                .collect();
            ui.horizontal(|ui| {
                let total = self.pages.iter().filter(|p| p.featured).count();
                ui.label(RichText::new(format!("{} shown, {} featured of {}", visible.len(), total, self.pages.len())).small().weak());
                if ui.small_button("Feature all shown").clicked() {
                    bulk = Some((visible.iter().map(|&i| self.pages[i].slug.clone()).collect(), true));
                }
                if ui.small_button("Unfeature all shown").clicked() {
                    bulk = Some((visible.iter().map(|&i| self.pages[i].slug.clone()).collect(), false));
                }
            });
            ui.separator();
            let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
            egui::ScrollArea::vertical().id_salt("featured-list").auto_shrink([false, false]).show_rows(ui, row_h, visible.len(), |ui, range| {
                for row in range {
                    let p = &self.pages[visible[row]];
                    let mut on = p.featured;
                    if ui.checkbox(&mut on, &p.title).on_hover_text(format!("pages/{}.md", p.slug)).changed() {
                        changes.push((p.slug.clone(), on));
                    }
                }
            });
        });
        if !open {
            self.show_featured = false;
        }
        if let Some((slugs, on)) = bulk {
            self.set_featured(&slugs, on);
        }
        let on: Vec<String> = changes.iter().filter(|c| c.1).map(|c| c.0.clone()).collect();
        let off: Vec<String> = changes.iter().filter(|c| !c.1).map(|c| c.0.clone()).collect();
        self.set_featured(&on, true);
        self.set_featured(&off, false);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.featured_window(ctx);
        if self.confirm_delete {
            let mut open = true;
            let mut close = false;
            egui::Window::new("Delete page?").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
                ui.label(format!("Delete pages/{}.md from disk? This cannot be undone.", self.current.clone().unwrap_or_default()));
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        self.delete_current();
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if !open || close {
                self.confirm_delete = false;
            }
        }
    }
}

impl App {
    fn update_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }

        egui::Panel::top("menubar").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("statusbar").show(ui, |ui| {
            ui.label(RichText::new(&self.status).small());
        });
        if self.site.is_some() {
            egui::Panel::left("pages-panel").default_size(240.0).resizable(true).show(ui, |ui| self.sidebar(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if self.site.is_none() {
                self.welcome(ui);
            } else {
                self.editor(ui);
            }
        });
        self.dialogs(&ctx);

        // auto-save once the user has been idle for a moment
        if self.current.is_some() {
            if self.is_dirty() {
                if ctx.input(|i| i.pointer.any_down() || !i.events.is_empty()) {
                    self.last_change = Instant::now();
                }
                if self.autosave && self.last_change.elapsed() > Duration::from_secs(3) {
                    self.save();
                }
                ctx.request_repaint_after(Duration::from_millis(500));
            } else {
                self.last_change = Instant::now();
            }
        }
    
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_ui(ui);
    }
}

// ---------------------------------------------------------------- block widgets

fn draw_block(ui: &mut egui::Ui, b: &mut Block, idx: usize, count: usize, site: &Site, action: &mut Option<Action>, status: &mut String) {
    ui.horizontal_wrapped(|ui| {
        ui.dnd_drag_source(egui::Id::unique(("drag", b.id)), idx, |ui| {
            ui.label(RichText::new("Drag").small().weak());
        })
        .response
        .on_hover_text("Drag to reorder");
        ui.label(RichText::new(b.kind.label()).strong().color(kind_color(b.kind)));
        ui.add_space(8.0);
        if ui.add_enabled(idx > 0, egui::Button::new("Up").small()).clicked() {
            *action = Some(Action::Up(idx));
        }
        if ui.add_enabled(idx + 1 < count, egui::Button::new("Down").small()).clicked() {
            *action = Some(Action::Down(idx));
        }
        if ui.add(egui::Button::new("Duplicate").small()).clicked() {
            *action = Some(Action::Duplicate(idx));
        }
        ui.menu_button("Insert below", |ui| {
            for k in ADD_KINDS {
                if ui.button(k.label()).clicked() {
                    ui.close();
                    *action = Some(Action::Insert(idx + 1, *k));
                }
            }
        });
        if ui.add(egui::Button::new("Delete").small()).clicked() {
            *action = Some(Action::Delete(idx));
        }
    });
    ui.add_space(2.0);

    match b.kind {
        Kind::Heading => heading_ui(ui, b),
        Kind::Text => text_ui(ui, b, idx, action),
        Kind::Figure => figure_ui(ui, b, site, status),
        Kind::Table => {
            ui.label(RichText::new("Static table (the first row is the header). Rendered without interaction.").small().weak());
            grid_editor(ui, b.id, &mut b.grid, false);
        }
        Kind::Sheet => sheet_ui(ui, b),
        Kind::Math => {
            ui.label(RichText::new("LaTeX, rendered as a centered display equation. Use $...$ inside text blocks for inline math.").small().weak());
            multiline(ui, &mut b.text, b.id, 3, true);
        }
        Kind::Code => code_ui(ui, b),
        Kind::Desmos => {
            ui.label(RichText::new("One Desmos expression (LaTeX) per line. Optional: bounds: left, right, bottom, top").small().weak());
            multiline(ui, &mut b.text, b.id, 5, true);
            height_row(ui, b, 400);
        }
        Kind::Map => {
            ui.label(RichText::new("Lines: center: lat, lon | zoom: n | marker: lat, lon | label | circle: lat, lon, radius_m | label | line: lat,lon; lat,lon | label").small().weak());
            multiline(ui, &mut b.text, b.id, 5, true);
            interactive_row(ui, b, "Interactive (drag, zoom, popups). Untick for a fixed picture with the labels shown.");
            height_row(ui, b, 360);
        }
        Kind::Model => model_ui(ui, b, site, status),
        Kind::Timeline => {
            ui.label(RichText::new("One event per line: date | title | description. Dates can be years (negative or '44 BC' for before the common era) or YYYY-MM-DD; events are spaced by date when every date is numeric.").small().weak());
            multiline(ui, &mut b.text, b.id, 6, true);
            interactive_row(ui, b, "Interactive (click events, previous / next, drag to scroll). Untick for a plain list.");
        }
        Kind::Audio => audio_ui(ui, b, site, status),
        Kind::CsvSearch => csv_ui(ui, b, site, status),
        Kind::Infobox => {
            ui.label(RichText::new("Lines of 'Name: value'. Special keys: title, image. Floats to the right of the article.").small().weak());
            multiline(ui, &mut b.text, b.id, 5, false);
        }
        Kind::Callout => {
            ui.horizontal(|ui| {
                ui.label("Style");
                let cur = b.attr("variant").to_string();
                egui::ComboBox::from_id_salt(("variant", b.id)).selected_text(if cur.is_empty() { "note" } else { &cur }).show_ui(ui, |ui| {
                    for v in ["note", "tip", "warning"] {
                        if ui.selectable_label(cur == v, v).clicked() {
                            b.set_attr("variant", v);
                        }
                    }
                });
            });
            multiline(ui, &mut b.text, b.id, 3, false);
        }
        Kind::Footnote => {
            ui.horizontal(|ui| {
                ui.label("Footnote id");
                let mut id = b.attr("id").to_string();
                if ui.add(egui::TextEdit::singleline(&mut id).desired_width(60.0)).changed() {
                    b.set_attr("id", id.replace(' ', ""));
                }
                ui.label(RichText::new(format!("Reference it in text as [^{}]", b.attr("id"))).small().weak());
            });
            multiline(ui, &mut b.text, b.id, 2, false);
        }
        Kind::Raw => {
            ui.label(RichText::new("Raw Markdown, written to the file exactly as typed.").small().weak());
            multiline(ui, &mut b.text, b.id, 4, true);
        }
    }
}

fn kind_color(k: Kind) -> Color32 {
    match k {
        Kind::Heading | Kind::Text => Color32::from_rgb(70, 110, 190),
        Kind::Figure | Kind::Model | Kind::Audio => Color32::from_rgb(40, 140, 100),
        Kind::Timeline | Kind::CsvSearch => Color32::from_rgb(190, 120, 30),
        Kind::Code | Kind::Raw => Color32::from_rgb(160, 80, 160),
        Kind::Table | Kind::Sheet => Color32::from_rgb(190, 120, 30),
        _ => Color32::from_rgb(110, 110, 120),
    }
}

fn multiline(ui: &mut egui::Ui, text: &mut String, id: u64, rows: usize, mono: bool) -> egui::Response {
    let mut te = egui::TextEdit::multiline(text).id(text_id(id)).desired_width(f32::INFINITY).desired_rows(rows);
    if mono {
        te = te.font(egui::TextStyle::Monospace);
    }
    ui.add(te)
}

fn height_row(ui: &mut egui::Ui, b: &mut Block, default: u32) {
    ui.horizontal(|ui| {
        ui.label("Height");
        let mut h: u32 = b.attr("height").parse().unwrap_or(default);
        if ui.add(egui::DragValue::new(&mut h).range(120..=1200).suffix(" px")).changed() {
            b.set_attr("height", h.to_string());
        }
    });
}

fn heading_ui(ui: &mut egui::Ui, b: &mut Block) {
    let mut level: u8 = b.attr("level").parse().unwrap_or(2);
    ui.horizontal(|ui| {
        ui.label("Level");
        egui::ComboBox::from_id_salt(("level", b.id)).selected_text(format!("H{level}")).show_ui(ui, |ui| {
            for l in 1..=4u8 {
                let label = match l {
                    1 => "H1 - article title",
                    2 => "H2 - section",
                    3 => "H3 - subsection",
                    _ => "H4 - sub-subsection",
                };
                if ui.selectable_label(level == l, label).clicked() {
                    level = l;
                }
            }
        });
    });
    b.set_attr("level", level.to_string());
    let size = match level {
        1 => 30.0,
        2 => 24.0,
        3 => 19.0,
        _ => 16.0,
    };
    let family = FontFamily::Name("serif".into());
    ui.add(
        egui::TextEdit::singleline(&mut b.text)
            .id(text_id(b.id))
            .desired_width(f32::INFINITY)
            .font(FontId::new(size, if level <= 2 { family } else { FontFamily::Proportional })),
    );
}

fn font_id_for(font: &str, size: f32) -> FontId {
    let size = if size > 0.0 { size.clamp(8.0, 72.0) } else { 14.0 };
    let f = font.to_lowercase();
    let fam = match f.as_str() {
        "serif" | "georgia" | "times new roman" | "palatino" | "times" => FontFamily::Name("serif".into()),
        "mono" | "monospace" | "courier new" | "courier" => FontFamily::Monospace,
        _ => FontFamily::Proportional,
    };
    FontId::new(size, fam)
}

fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn wrap_selection(ctx: &egui::Context, id: egui::Id, text: &mut String, before: &str, after: &str, placeholder: &str) {
    let chars: Vec<char> = text.chars().collect();
    let (a, b) = match egui::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range()) {
        Some(r) => {
            let [lo, hi] = r.sorted_cursors();
            (lo.index.0.min(chars.len()), hi.index.0.min(chars.len()))
        }
        None => (chars.len(), chars.len()),
    };
    let selected: String = chars[a..b].iter().collect();
    let middle = if selected.is_empty() { placeholder.to_string() } else { selected };
    let mut out: String = chars[..a].iter().collect();
    out.push_str(before);
    out.push_str(&middle);
    out.push_str(after);
    out.extend(chars[b..].iter());
    *text = out;
    let end = a + before.chars().count() + middle.chars().count() + after.chars().count();
    if let Some(mut st) = egui::TextEdit::load_state(ctx, id) {
        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(end))));
        st.store(ctx, id);
    }
}

fn text_ui(ui: &mut egui::Ui, b: &mut Block, idx: usize, action: &mut Option<Action>) {
    // --- style row (click-to-edit text with font, size, color and alignment)
    ui.horizontal_wrapped(|ui| {
        let cur_font = if b.attr("font").is_empty() { "(default)".to_string() } else { b.attr("font").to_string() };
        egui::ComboBox::from_id_salt(("font", b.id)).selected_text(cur_font.clone()).width(130.0).show_ui(ui, |ui| {
            for f in FONT_CHOICES {
                if ui.selectable_label(cur_font == *f, *f).clicked() {
                    b.set_attr("font", if *f == "(default)" { "" } else { f });
                }
            }
        });
        let mut size: f32 = b.attr("size").parse().unwrap_or(0.0);
        if ui.add(egui::DragValue::new(&mut size).range(0.0..=72.0).speed(0.4).custom_formatter(|v, _| if v < 1.0 { "default size".into() } else { format!("{v:.0} px") })).changed() {
            b.set_attr("size", if size < 1.0 { String::new() } else { format!("{size:.0}") });
        }
        let mut color = parse_hex(b.attr("color")).unwrap_or(ui.visuals().text_color());
        if egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::Opaque).changed() {
            b.set_attr("color", to_hex(color));
        }
        let cur_align = if b.attr("align").is_empty() { "left" } else { b.attr("align") }.to_string();
        egui::ComboBox::from_id_salt(("align", b.id)).selected_text(cur_align.clone()).width(80.0).show_ui(ui, |ui| {
            for a in ["left", "center", "right", "justify"] {
                if ui.selectable_label(cur_align == a, a).clicked() {
                    b.set_attr("align", if a == "left" { "" } else { a });
                }
            }
        });
        if ui.small_button("Reset style").clicked() {
            for k in ["font", "size", "color", "align"] {
                b.attrs.remove(k);
            }
        }
    });

    // --- text area, drawn with the chosen font so the editor looks like the page
    let size: f32 = b.attr("size").parse().unwrap_or(0.0);
    let font = font_id_for(b.attr("font"), size);
    let color = parse_hex(b.attr("color"));
    let align = match b.attr("align") {
        "center" => egui::Align2::CENTER_TOP,
        "right" => egui::Align2::RIGHT_TOP,
        _ => egui::Align2::LEFT_TOP,
    };
    let id = text_id(b.id);
    let mut te = egui::TextEdit::multiline(&mut b.text)
        .id(id)
        .desired_width(f32::INFINITY)
        .desired_rows(3)
        .font(font)
        .align(align);
    if let Some(c) = color {
        te = te.text_color(c);
    }
    ui.add(te);

    // --- Markdown helpers that wrap the current selection
    let ctx = ui.ctx().clone();
    ui.horizontal_wrapped(|ui| {
        let mut wrap = |ui: &mut egui::Ui, label: &str, tip: &str, before: &str, after: &str, ph: &str| {
            if ui.small_button(label).on_hover_text(tip).clicked() {
                wrap_selection(&ctx, id, &mut b.text, before, after, ph);
            }
        };
        wrap(ui, "Bold", "**bold**", "**", "**", "bold text");
        wrap(ui, "Italic", "*italic*", "*", "*", "italic text");
        wrap(ui, "Code", "`code`", "`", "`", "code");
        wrap(ui, "Page link", "Blue link to another article: [[page]]", "[[", "]]", "Page name");
        wrap(ui, "Web link", "Green link to an outside site", "[", "](https://example.com)", "link text");
        wrap(ui, "Inline math", "$LaTeX$", "$", "$", "x^2");
        if ui.small_button("Footnote").on_hover_text("Inserts [^n] here and adds a footnote block at the end").clicked() {
            *action = Some(Action::FootnoteRef(idx));
        }
    });
}

fn media_row(ui: &mut egui::Ui, label: &str, b: &mut Block, key: &str, site: &Site, exts: &[&str], status: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut v = b.attr(key).to_string();
        if ui.add(egui::TextEdit::singleline(&mut v).desired_width(300.0).hint_text("media/file.png or https://...")).changed() {
            b.set_attr(key, v);
        }
        if ui.button("Browse...").clicked() {
            if let Some(path) = rfd::FileDialog::new().add_filter("Supported files", exts).pick_file() {
                match site.import_media(&path) {
                    Ok(rel) => b.set_attr(key, rel),
                    Err(e) => *status = format!("Could not copy the file: {e}"),
                }
            }
        }
    });
}

fn figure_ui(ui: &mut egui::Ui, b: &mut Block, site: &Site, status: &mut String) {
    let mut exts: Vec<&str> = Vec::new();
    exts.extend(IMAGE_EXTS);
    exts.extend(VIDEO_EXTS);
    media_row(ui, "Source", b, "src", site, &exts, status);
    ui.horizontal(|ui| {
        ui.label("Caption");
        let mut v = b.attr("caption").to_string();
        if ui.add(egui::TextEdit::singleline(&mut v).desired_width(f32::INFINITY).hint_text("Optional caption (Markdown allowed)")).changed() {
            b.set_attr("caption", v);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Alt text");
        let mut alt = b.attr("alt").to_string();
        if ui.add(egui::TextEdit::singleline(&mut alt).desired_width(200.0)).changed() {
            b.set_attr("alt", alt);
        }
        ui.label("Position");
        let cur = if b.attr("align").is_empty() { "right" } else { b.attr("align") }.to_string();
        egui::ComboBox::from_id_salt(("fig-align", b.id)).selected_text(cur.clone()).width(90.0).show_ui(ui, |ui| {
            for a in ["right", "left", "center", "none"] {
                if ui.selectable_label(cur == a, a).clicked() {
                    b.set_attr("align", a);
                }
            }
        });
        ui.label("Width");
        let mut w: u32 = b.attr("width").parse().unwrap_or(0);
        if ui.add(egui::DragValue::new(&mut w).range(0..=1600).speed(2.0).custom_formatter(|v, _| if v < 1.0 { "auto".into() } else { format!("{v:.0} px") })).changed() {
            b.set_attr("width", if w == 0 { String::new() } else { w.to_string() });
        }
    });

    let src = b.attr("src").to_string();
    let ext = ext_of(&src);
    if src.is_empty() {
        ui.label(RichText::new("No file chosen yet.").weak());
    } else if src.starts_with("http") {
        ui.label(RichText::new("Remote file; shown in the browser preview.").small().weak());
    } else if VIDEO_EXTS.contains(&ext.as_str()) {
        ui.label(RichText::new(format!("Video: {src} (plays in the browser preview)")).small().weak());
    } else if IMAGE_EXTS.contains(&ext.as_str()) {
        let path = site.media_path(&src);
        if path.is_file() {
            ui.add(egui::Image::new(format!("file://{}", path.display())).max_width(320.0).max_height(200.0));
        } else {
            ui.colored_label(Color32::from_rgb(190, 60, 60), format!("{src} was not found in the site folder."));
        }
    }
}

fn model_ui(ui: &mut egui::Ui, b: &mut Block, site: &Site, status: &mut String) {
    ui.label(RichText::new("Interactive 3D viewer for glTF models (.glb / .gltf). Drag to rotate in the page.").small().weak());
    media_row(ui, "Model", b, "src", site, MODEL_EXTS, status);
    ui.horizontal_wrapped(|ui| {
        ui.label("Alt text");
        let mut alt = b.attr("alt").to_string();
        if ui.add(egui::TextEdit::singleline(&mut alt).desired_width(220.0)).changed() {
            b.set_attr("alt", alt);
        }
        let mut rotate = b.attr("rotate") != "false";
        if ui.checkbox(&mut rotate, "Auto-rotate").changed() {
            b.set_attr("rotate", if rotate { "" } else { "false" });
        }
    });
    height_row(ui, b, 420);
}

/// Checkbox for the `interactive` attribute. On is the default and is not written to the file;
/// off is stored as `interactive=false`.
fn interactive_row(ui: &mut egui::Ui, b: &mut Block, label: &str) {
    let mut on = !matches!(b.attr("interactive"), "false" | "0");
    if ui.checkbox(&mut on, label).changed() {
        b.set_attr("interactive", if on { "" } else { "false" });
    }
}

fn audio_ui(ui: &mut egui::Ui, b: &mut Block, site: &Site, status: &mut String) {
    ui.label(RichText::new("A small audio player. Choose a sound file (mp3, wav, ogg, m4a, flac ...) or paste an address.").small().weak());
    media_row(ui, "Audio", b, "src", site, AUDIO_EXTS, status);
    ui.horizontal(|ui| {
        ui.label("Title");
        let mut v = b.attr("title").to_string();
        if ui.add(egui::TextEdit::singleline(&mut v).desired_width(f32::INFINITY).hint_text("Optional, shown above the player")).changed() {
            b.set_attr("title", v);
        }
    });
    let src = b.attr("src");
    if src.is_empty() {
        ui.label(RichText::new("No file chosen yet.").weak());
    } else if !src.starts_with("http") && !site.media_path(src).is_file() {
        ui.colored_label(Color32::from_rgb(190, 60, 60), format!("{src} was not found in the site folder."));
    } else {
        ui.label(RichText::new("Plays in the browser preview.").small().weak());
    }
}

/// First rows of a CSV file, cached in egui's temporary memory so the file is read once rather
/// than on every repaint (the file may have thousands of rows).
#[derive(Clone)]
struct CsvPreview {
    src: String,
    rows: Vec<Vec<String>>,
    total: usize,
    error: Option<String>,
}

fn load_csv_preview(site: &Site, src: &str) -> CsvPreview {
    let mut p = CsvPreview { src: src.to_string(), rows: Vec::new(), total: 0, error: None };
    match std::fs::read_to_string(site.media_path(src)) {
        Ok(text) => {
            let all = parse_csv(&text);
            p.total = all.len().saturating_sub(1);
            p.rows = all.into_iter().take(6).collect();
        }
        Err(e) => p.error = Some(format!("Could not read {src}: {e}")),
    }
    p
}

fn csv_ui(ui: &mut egui::Ui, b: &mut Block, site: &Site, status: &mut String) {
    ui.label(RichText::new("Upload a CSV file (the first row is the header). Readers get a search box for every column, plus one for all columns, and can sort by clicking a header.").small().weak());
    ui.label(RichText::new("In a column box: text matches anywhere in the cell, !text excludes it, >100  <=5  =7 compare numbers, 10..20 is a range of numbers.").small().weak());
    media_row(ui, "CSV file", b, "src", site, CSV_EXTS, status);
    let src = b.attr("src").to_string();
    let pid = egui::Id::unique(("csv-preview", b.id));
    if src.is_empty() {
        ui.label(RichText::new("No file chosen yet.").weak());
    } else if src.starts_with("http") {
        ui.label(RichText::new("Remote file; the server must allow cross-origin requests. Shown in the browser preview.").small().weak());
    } else {
        let cached = ui.ctx().data(|d| d.get_temp::<CsvPreview>(pid)).filter(|p| p.src == src);
        let preview = match cached {
            Some(p) => p,
            None => {
                let p = load_csv_preview(site, &src);
                ui.ctx().data_mut(|d| d.insert_temp(pid, p.clone()));
                p
            }
        };
        if let Some(e) = &preview.error {
            ui.colored_label(Color32::from_rgb(190, 60, 60), e);
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} data rows, {} columns. First rows:", preview.total, preview.rows.first().map(|r| r.len()).unwrap_or(0))).small().weak());
                if ui.small_button("Reload").clicked() {
                    ui.ctx().data_mut(|d| d.remove::<CsvPreview>(pid));
                }
            });
            egui::ScrollArea::horizontal().id_salt(("csv-preview-scroll", b.id)).show(ui, |ui| {
                egui::Grid::new(("csv-preview-grid", b.id)).striped(true).show(ui, |ui| {
                    for (r, row) in preview.rows.iter().enumerate() {
                        for cell in row {
                            let t = RichText::new(cell).small();
                            ui.label(if r == 0 { t.strong() } else { t });
                        }
                        ui.end_row();
                    }
                });
            });
        }
    }
    ui.horizontal(|ui| {
        ui.label("Results height");
        let mut h: u32 = b.attr("height").parse().unwrap_or(360);
        if ui.add(egui::DragValue::new(&mut h).range(120..=1200).suffix(" px")).changed() {
            b.set_attr("height", h.to_string());
        }
    });
}

fn sheet_ui(ui: &mut egui::Ui, b: &mut Block) {
    ui.label(RichText::new("Interactive spreadsheet. Cells starting with = are formulas; separate function arguments with ; (for example =IF(A1>2;\"big\";\"small\")).").small().weak());
    grid_editor(ui, b.id, &mut b.grid, true);
    ui.horizontal(|ui| {
        if ui.button("Import CSV...").clicked() {
            if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv", "txt"]).pick_file() {
                if let Ok(text) = std::fs::read_to_string(path) {
                    b.grid = parse_csv(&text);
                }
            }
        }
        ui.label("Height");
        let mut h: u32 = b.attr("height").parse().unwrap_or(0);
        if ui.add(egui::DragValue::new(&mut h).range(0..=1200).custom_formatter(|v, _| if v < 1.0 { "auto".into() } else { format!("{v:.0} px") })).changed() {
            b.set_attr("height", if h == 0 { String::new() } else { h.to_string() });
        }
    });
}

fn col_name(mut n: usize) -> String {
    let mut s = String::new();
    n += 1;
    while n > 0 {
        let m = (n - 1) % 26;
        s.insert(0, (b'A' + m as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

fn grid_editor(ui: &mut egui::Ui, id: u64, grid: &mut Vec<Vec<String>>, spreadsheet: bool) {
    rectangular(grid);
    let rows = grid.len();
    let cols = grid[0].len();
    egui::ScrollArea::horizontal().id_salt(("grid-scroll", id)).show(ui, |ui| {
        egui::Grid::new(("grid", id)).striped(true).spacing([4.0, 4.0]).show(ui, |ui| {
            if spreadsheet {
                ui.label("");
                for c in 0..cols {
                    ui.label(RichText::new(col_name(c)).weak());
                }
                ui.end_row();
            }
            for r in 0..rows {
                if spreadsheet {
                    ui.label(RichText::new((r + 1).to_string()).weak());
                }
                for c in 0..cols {
                    let mut te = egui::TextEdit::singleline(&mut grid[r][c]).desired_width(96.0);
                    if !spreadsheet && r == 0 {
                        te = te.text_color(ui.visuals().strong_text_color());
                    }
                    ui.add(te);
                }
                ui.end_row();
            }
        });
    });
    ui.horizontal(|ui| {
        if ui.small_button("+ Row").clicked() {
            grid.push(vec![String::new(); cols]);
        }
        if ui.add_enabled(rows > 1, egui::Button::new("- Row").small()).clicked() {
            grid.pop();
        }
        if ui.small_button("+ Column").clicked() {
            for r in grid.iter_mut() {
                r.push(String::new());
            }
        }
        if ui.add_enabled(cols > 1, egui::Button::new("- Column").small()).clicked() {
            for r in grid.iter_mut() {
                r.pop();
            }
        }
    });
}

fn starter(lang: &str, canvas: bool) -> &'static str {
    match (lang, canvas) {
        ("python", false) => "print(\"Hello, world!\")\nfor i in range(5):\n    print(i * i)\n",
        ("python", true) => "import turtle\n\nrect(20, 20, 120, 80, \"#3366cc\")\nellipse(260, 70, 70, 40, \"#dd5522\")\ntext(160, 40, \"Hello, canvas!\")\nturtle.color(\"#228833\")\nfor i in range(36):\n    turtle.forward(120)\n    turtle.left(170)\n",
        ("javascript", false) => "console.log(\"Hello, world!\");\nfor (let i = 0; i < 5; i++) console.log(i * i);\n",
        ("javascript", true) => "rect(20, 20, 120, 80, \"#3366cc\");\nellipse(260, 70, 70, 40, \"#dd5522\");\ntext(160, 40, \"Hello, canvas!\");\ncolor(\"#228833\");\nfor (let i = 0; i < 36; i++) {\n  forward(120);\n  left(170);\n}\n",
        ("c", false) => "#include <stdio.h>\n\nint main(void) {\n    printf(\"Hello, world!\\n\");\n    return 0;\n}\n",
        ("c", true) => "#include <stdio.h>\n\nint main(void) {\n    printf(\"@rect 20 20 120 80 #3366cc\\n\");\n    printf(\"@ellipse 260 70 70 40 #dd5522\\n\");\n    printf(\"@color #228833\\n\");\n    for (int i = 0; i < 36; i++) {\n        printf(\"@forward 120\\n@left 170\\n\");\n    }\n    return 0;\n}\n",
        ("cpp", false) => "#include <iostream>\n\nint main() {\n    std::cout << \"Hello, world!\\n\";\n}\n",
        ("cpp", true) => "#include <iostream>\n\nint main() {\n    std::cout << \"@rect 20 20 120 80 #3366cc\\n\";\n    std::cout << \"@ellipse 260 70 70 40 #dd5522\\n\";\n    std::cout << \"@color #228833\\n\";\n    for (int i = 0; i < 36; i++) {\n        std::cout << \"@forward 120\\n@left 170\\n\";\n    }\n}\n",
        ("rust", false) => "fn main() {\n    println!(\"Hello, world!\");\n}\n",
        ("rust", true) => "fn main() {\n    println!(\"@rect 20 20 120 80 #3366cc\");\n    println!(\"@ellipse 260 70 70 40 #dd5522\");\n    println!(\"@color #228833\");\n    for _ in 0..36 {\n        println!(\"@forward 120\");\n        println!(\"@left 170\");\n    }\n}\n",
        ("java", false) => "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"Hello, world!\");\n    }\n}\n",
        ("java", true) => "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"@rect 20 20 120 80 #3366cc\");\n        System.out.println(\"@ellipse 260 70 70 40 #dd5522\");\n        System.out.println(\"@color #228833\");\n        for (int i = 0; i < 36; i++) {\n            System.out.println(\"@forward 120\");\n            System.out.println(\"@left 170\");\n        }\n    }\n}\n",
        _ => "Plain text\n",
    }
}

fn code_ui(ui: &mut egui::Ui, b: &mut Block) {
    let lang = if b.attr("lang").is_empty() { "text".to_string() } else { b.attr("lang").to_string() };
    let runnable = lang != "text";
    ui.horizontal_wrapped(|ui| {
        ui.label("Language");
        let label = CODE_LANGS.iter().find(|(k, _)| *k == lang).map(|(_, l)| *l).unwrap_or(lang.as_str());
        egui::ComboBox::from_id_salt(("lang", b.id)).selected_text(label).show_ui(ui, |ui| {
            for (k, l) in CODE_LANGS {
                if ui.selectable_label(lang == *k, *l).clicked() {
                    b.set_attr("lang", *k);
                }
            }
        });
        let mut canvas = b.attr("canvas") == "1";
        if ui.add_enabled(runnable, egui::Checkbox::new(&mut canvas, "Include canvas")).on_hover_text("Adds a drawing canvas with a turtle, rect, ellipse, line and text commands").changed() {
            b.set_attr("canvas", if canvas { "1" } else { "" });
        }
        if ui.small_button("Insert starter code").on_hover_text("Replaces the code with a starter for this language").clicked() {
            b.text = starter(&lang, b.attr("canvas") == "1").trim_end().to_string();
        }
    });
    if b.attr("canvas") == "1" && runnable {
        let hint = match lang.as_str() {
            "python" | "javascript" => "Canvas: rect, ellipse, circle, line, text, clear, forward, back, left, right, penup, pendown, goto, color, pensize, dot",
            _ => "Canvas: print command lines to stdout, e.g. @rect 10 10 80 40 #36c  or  @forward 100  (see the showcase page)",
        };
        ui.label(RichText::new(hint).small().weak());
    }
    if matches!(lang.as_str(), "c" | "cpp" | "java" | "rust") {
        ui.label(RichText::new("Compiled and run by a public online service when a reader presses Run.").small().weak());
    }
    multiline(ui, &mut b.text, b.id, 9, true);
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "C:\\Windows\\Fonts\\georgia.ttf",
        "C:\\Windows\\Fonts\\times.ttf",
        "/System/Library/Fonts/Supplemental/Georgia.ttf",
        "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSerif.ttf",
        "/usr/share/fonts/dejavu/DejaVuSerif.ttf",
    ];
    let mut family: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for path in candidates {
        if let Ok(bytes) = std::fs::read(PathBuf::from(path)) {
            fonts.font_data.insert("system-serif".to_string(), Arc::new(egui::FontData::from_owned(bytes)));
            family.insert(0, "system-serif".to_string());
            break;
        }
    }
    fonts.families.insert(FontFamily::Name("serif".into()), family);
    ctx.set_fonts(fonts);
}


#[cfg(test)]
impl App {
    fn headless() -> Self {
        App {
            site: None,
            pages: Vec::new(),
            current: None,
            blocks: Vec::new(),
            saved_md: String::new(),
            status: String::new(),
            new_page_name: String::new(),
            server: None,
            confirm_delete: false,
            autosave: false,
            last_change: Instant::now(),
            page_filter: String::new(),
            show_featured: false,
            featured_filter: String::new(),
            featured_only: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, app: &mut App) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1300.0, 900.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| app.update_ui(ui));
        out.textures_delta.clear();
    }

    #[test]
    fn editor_renders_every_block_type() {
        let dir = std::env::temp_dir().join(format!("wikiforge-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Site::create(&dir, "UI test").unwrap().write_showcase().unwrap();

        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        egui_extras::install_image_loaders(&ctx);
        let mut app = App::headless();

        frame(&ctx, &mut app); // welcome screen
        app.load_site(Site::open(&dir).unwrap());
        app.select_page("showcase");
        assert!(app.blocks.len() > 30);
        assert!(!app.is_dirty());
        for _ in 0..3 {
            frame(&ctx, &mut app);
        }

        // one block of every kind, plus the editing actions
        for k in ADD_KINDS {
            app.apply(&ctx, Action::Insert(0, *k));
        }
        app.apply(&ctx, Action::Duplicate(1));
        app.apply(&ctx, Action::Down(0));
        app.apply(&ctx, Action::Up(3));
        app.apply(&ctx, Action::Move(0, 5));
        app.apply(&ctx, Action::FootnoteRef(app.blocks.iter().position(|b| b.kind == Kind::Text).unwrap()));
        for _ in 0..3 {
            frame(&ctx, &mut app);
        }
        assert!(app.is_dirty());
        app.save();
        assert!(!app.is_dirty());

        // what was saved must parse back to the same structure
        let md = std::fs::read_to_string(dir.join("pages/showcase.md")).unwrap();
        let reparsed = parse_markdown(&md);
        if reparsed.len() != app.blocks.len() {
            for (i, b) in app.blocks.iter().enumerate() {
                eprintln!("{i}: {:?} -> {:?}", b.kind, reparsed.get(i).map(|r| r.kind));
            }
        }
        assert_eq!(reparsed.len(), app.blocks.len());

        app.create_page_named("Second page");
        frame(&ctx, &mut app);
        assert!(dir.join("pages/second-page.md").is_file());
        let manifest = std::fs::read_to_string(dir.join("pages/pages.json")).unwrap();
        assert!(manifest.contains("second-page") && manifest.contains("Second page"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn examples_and_featured_articles() {
        let dir = std::env::temp_dir().join(format!("wikiforge-ui-feat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Site::create(&dir, "Feat test").unwrap();
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        let mut app = App::headless();
        app.load_site(Site::open(&dir).unwrap());
        frame(&ctx, &mut app);

        app.add_examples();
        assert_eq!(app.pages.len(), 10);
        assert!(dir.join("pages/example-10.md").is_file());
        assert!(app.status.contains("10"), "{}", app.status);
        app.add_examples();
        assert!(app.status.contains("already exist"), "{}", app.status);

        // nothing is featured to begin with
        assert_eq!(app.pages.iter().filter(|p| p.featured).count(), 0);
        app.set_featured(&["example-1".to_string(), "example-2".to_string()], true);
        assert_eq!(app.pages.iter().filter(|p| p.featured).count(), 2);
        assert!(std::fs::read_to_string(dir.join("pages/pages.json")).unwrap().contains("\"featured\": [\"example-1\", \"example-2\"]"));

        // the featured window and the filtered sidebar render, with and without a filter
        app.show_featured = true;
        frame(&ctx, &mut app);
        app.featured_filter = "example 1".into();
        app.featured_only = true;
        app.page_filter = "example".into();
        frame(&ctx, &mut app);
        assert!(app.show_featured);

        app.set_featured(&["example-1".to_string()], false);
        assert_eq!(app.pages.iter().filter(|p| p.featured).count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrap_selection_appends_without_cursor() {
        let ctx = egui::Context::default();
        let mut t = String::from("hello");
        wrap_selection(&ctx, egui::Id::unique("none"), &mut t, "**", "**", "bold");
        assert_eq!(t, "hello**bold**");
    }
}
