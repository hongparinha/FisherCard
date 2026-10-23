#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod model;
mod schedule;
mod similarity;
mod srs;
mod store;
mod theme;
mod timeutil;
mod ui;

use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("FisherCard")
            .with_app_id("FisherCard")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 660.0])
            .with_decorations(false)
            .with_transparent(false)
            .with_resizable(true)
            .with_has_shadow(true)
            .with_icon(Arc::new(ui::app_icon())),
        centered: true,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "FisherCard",
        options,
        Box::new(|cc| Ok(Box::new(app::FisherApp::new(cc)))),
    )
}
