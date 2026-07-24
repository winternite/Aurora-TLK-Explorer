#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

use app::AuroraApp;
use eframe::egui;

fn main() -> eframe::Result {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/ateicon.png"))
        .expect("bundled Aurora icon must be a valid PNG");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("org.aurora_tools.AuroraTlkExplorer")
            .with_title("Aurora TLK Explorer")
            .with_icon(icon)
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([820.0, 560.0]),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "org.aurora_tools.AuroraTlkExplorer",
        options,
        Box::new(|cc| Ok(Box::new(AuroraApp::new(cc)))),
    )
}
