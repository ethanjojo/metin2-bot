#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use clap::Parser;
use metin_desktop::{cli::Cli, storage};
use std::fs;

fn startup(cli: Cli) -> Result<(), String> {
    #[cfg(windows)]
    metin_platform::enable_dpi();
    if cli.self_test {
        return metin_desktop::self_test::run(
            cli.self_test_report
                .as_deref()
                .expect("clap requires report"),
        );
    }
    let explicit = cli.data_dir.is_some();
    let root = match cli.data_dir {
        Some(path) => path,
        None => storage::default_data_directory().map_err(|e| e.to_string())?,
    };
    storage::validate_data_directory(&root).map_err(|e| e.to_string())?;
    let empty = !root.exists()
        || fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .next()
            .is_none();
    let mut import = cli.import_data;
    if !explicit && empty && import.is_none() {
        let adjacent = storage::legacy_data_directory().map_err(|e| e.to_string())?;
        if storage::has_legacy_data(&adjacent) {
            import = Some(adjacent);
        } else {
            let choice = rfd::MessageDialog::new()
                .set_title("Welcome to MetinBot")
                .set_description("Import profiles and settings from an older MetinBot folder? Choose No to start fresh, or Cancel to leave without creating data.")
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show();
            match choice {
                rfd::MessageDialogResult::Yes => {
                    import = rfd::FileDialog::new()
                        .set_title("Select the old MetinBot data folder")
                        .pick_folder();
                    if import.is_none() {
                        return Ok(());
                    }
                }
                rfd::MessageDialogResult::No => {}
                _ => return Ok(()),
            }
        }
    }
    if let Some(source) = import {
        storage::import_data(&source, &root).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let log = cli.log.unwrap_or_else(|| root.join("logs/run.log"));
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| e.to_string())?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::sync::Mutex::new(log_file))
        .try_init()
        .map_err(|e| e.to_string())?;
    tracing::info!(build=%metin_desktop::build_info(),"Starting MetinBot");
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("MetinBot")
        .with_inner_size([1280., 820.])
        .with_min_inner_size([800., 560.]);
    #[cfg(feature = "ui-smoke")]
    let viewport = if cli.ui_small {
        viewport.with_inner_size([800., 560.])
    } else {
        viewport
    };
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "MetinBot",
        options,
        Box::new(move |cc| {
            let app = metin_desktop::ui::MetinApp::new(cc, root).map_err(std::io::Error::other)?;
            #[cfg(feature = "ui-smoke")]
            let app = {
                let mut app = app;
                if let Some(path) = &cli.ui_fixture {
                    app.load_fixture(path).map_err(std::io::Error::other)?;
                }
                if let Some(path) = cli.ui_screenshot {
                    app.request_screenshot(path, cli.ui_tab);
                }
                app
            };
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())
}
fn main() {
    let cli = Cli::parse();
    let self_test = cli.self_test;
    if let Err(error) = startup(cli) {
        eprintln!("MetinBot: {error}");
        if !self_test {
            rfd::MessageDialog::new()
                .set_title("Unable to start MetinBot")
                .set_description(&error)
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
        std::process::exit(1);
    }
}
