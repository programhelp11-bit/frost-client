// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
use frost_client::ui::FrostApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Two worker threads are plenty: downloads are I/O bound, the UI has its own thread.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("frost-async")
        .build()
        .expect("tokio runtime");
    let handle = runtime.handle().clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Frost Client")
            .with_app_id("frost-launcher")
            .with_inner_size([1120.0, 700.0])
            .with_min_inner_size([920.0, 600.0])
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Frost Client",
        options,
        Box::new(move |cc| Ok(Box::new(FrostApp::new(cc, handle)))),
    );
    drop(runtime);
    result
}
