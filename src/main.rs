// A double-clicked Windows program should open the canvas directly, the same
// way the Linux program does, without a spare console window behind it.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 840.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "MindMap",
        options,
        Box::new(|cc| Ok(Box::new(app::MindMapApp::new(cc)))),
    );
    if let Err(error) = &result {
        record_startup_error(error);
    }
    result
}

/// Windows has no terminal when the window subsystem is used. Leave the
/// message beside the program so a failure to open the canvas is visible.
fn record_startup_error(error: &eframe::Error) {
    #[cfg(target_os = "windows")]
    {
        let path = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("mindmap-error.log")))
            .unwrap_or_else(|| std::path::PathBuf::from("mindmap-error.log"));
        let _ = std::fs::write(path, format!("{error}\n"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = error;
    }
}
