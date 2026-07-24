#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod instance;

use app::AuroraApp;
use eframe::egui;
#[cfg(target_os = "linux")]
use winit::platform::x11::EventLoopBuilderExtX11;

#[cfg(target_os = "linux")]
fn enable_kwin_placement() {
    const PLUGIN_NAME: &str = "aurora-active-output";

    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(prefix) = executable.parent().and_then(|bin| bin.parent()) else {
        return;
    };
    let script = prefix.join("share/aurora-tlk-explorer/kwin-active-output.js");
    if !script.is_file() {
        return;
    }

    let _ = std::process::Command::new("qdbus")
        .args([
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.unloadScript",
            PLUGIN_NAME,
        ])
        .output();
    let loaded = std::process::Command::new("qdbus")
        .args([
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.loadScript",
        ])
        .arg(&script)
        .arg(PLUGIN_NAME)
        .output()
        .is_ok_and(|output| output.status.success());
    if loaded {
        let _ = std::process::Command::new("qdbus")
            .args(["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"])
            .output();
    }
}

fn main() -> eframe::Result {
    let startup_paths = std::env::args_os().skip(1).map(Into::into).collect();
    #[cfg(target_os = "linux")]
    enable_kwin_placement();
    let incoming_paths = match instance::acquire_or_forward(startup_paths) {
        Ok(Some(receiver)) => receiver,
        Ok(None) => return Ok(()),
        Err(error) => {
            eprintln!("Could not hand off files to Aurora: {error}");
            return Ok(());
        }
    };
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/ateicon.png"))
        .expect("bundled Aurora icon must be a valid PNG");
    let viewport = egui::ViewportBuilder::default()
        .with_app_id("org.aurora_tools.AuroraTlkExplorer")
        .with_title("Aurora TLK Explorer")
        .with_icon(icon)
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([820.0, 560.0]);
    #[allow(unused_mut)] // Mutation is Linux-only, to choose XWayland for file drops.
    let mut options = eframe::NativeOptions {
        viewport,
        // KWin decides placement using the desktop startup/activation token.
        centered: false,
        // Restore the previous size and fullscreen/maximized state.
        persist_window: true,
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_some() {
        // Winit's Wayland backend does not currently implement file drops.
        // Use the available XWayland server, whose backend does, when both are
        // present (the normal configuration on desktop Linux).
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_x11();
        }));
    }
    eframe::run_native(
        "org.aurora_tools.AuroraTlkExplorer",
        options,
        Box::new(move |cc| Ok(Box::new(AuroraApp::new(cc, incoming_paths)))),
    )
}
