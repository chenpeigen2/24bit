//! Entry point for the desktop client.

use twentyfourbit_gui::app::App;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "twentyfourbit=info,warn".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([820.0, 520.0])
            .with_title("24bit — Hi-Res 无损音乐"),
        ..Default::default()
    };

    eframe::run_native(
        "24bit",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
