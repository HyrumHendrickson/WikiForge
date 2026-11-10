mod app;
mod model;
mod server;
mod site;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("WikiForge"),
        ..Default::default()
    };
    eframe::run_native("WikiForge", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
