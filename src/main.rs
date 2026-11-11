mod app;
mod model;
mod server;
mod site;

use eframe::egui;

/// The WikiForge logo as the window icon (skipped if it cannot be decoded).
fn app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(site::DEFAULT_LOGO).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([1320.0, 860.0]).with_min_inner_size([800.0, 500.0]).with_title("WikiForge");
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("WikiForge", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_icon_decodes() {
        let icon = super::app_icon().expect("the embedded logo decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
