#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use clap::Parser;
use metin_desktop::{cli::Cli, storage};
use std::fs;

fn startup(cli: Cli) -> Result<(), String> {
    #[cfg(windows)]
    metin_platform::enable_dpi();
    if let Some(operation) = cli.input_driver {
        let report = cli
            .input_driver_report
            .as_deref()
            .expect("clap requires report");
        if !report.is_absolute() {
            return Err("Input driver report path must be absolute.".into());
        }
        return metin_desktop::installation::run_reported_operation(report, || {
            #[cfg(windows)]
            { metin_platform::manage_input_driver(operation.operation()).map_err(|e| e.to_string()) }
            #[cfg(not(windows))]
            { Err::<String, String>(format!("{} input driver requires Windows x64.", metin_desktop::PRODUCT_NAME)) }
        }, |result| serde_json::json!({"operation":operation.name(),"success":result.is_ok(),"message":match result {Ok(message)|Err(message)=>message}})).map(|_| ());
    }
    if cli.self_test {
        return metin_desktop::self_test::run(
            cli.self_test_report
                .as_deref()
                .expect("clap requires report"),
        );
    }
    if let Some(report) = cli.migration_report.as_deref() {
        if !report.is_absolute() {
            return Err("Migration report path must be absolute.".into());
        }
        let protected = if let Some(previous) = &cli.finish_upgrade {
            vec![
                previous.clone(),
                std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .parent()
                    .ok_or("Executable has no parent.")?
                    .to_path_buf(),
            ]
        } else {
            storage::migration_protected_paths(cli.data_dir.as_deref())
                .map_err(|e| e.to_string())?
        };
        let mut report_file = metin_desktop::installation::reserve_migration_report(
            report,
            &protected.iter().map(|p| p.as_path()).collect::<Vec<_>>(),
        )?;
        let result = if let Some(previous) = cli.finish_upgrade {
            (|| {
                let executable = std::env::current_exe().map_err(|e| e.to_string())?;
                let current = executable
                    .parent()
                    .ok_or("Executable has no installation directory.")?;
                let snapshot = cli
                    .previous_inventory
                    .as_deref()
                    .expect("clap requires previous inventory");
                let backup = metin_desktop::installation::finalize_upgrade(
                    &previous,
                    current,
                    snapshot,
                    || {
                        let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
                        metin_desktop::self_test::run(&temporary.path().join("self-test.json"))
                    },
                )?;
                Ok(
                    serde_json::json!({"installation":current,"backup":backup,"self_test_success":true}),
                )
            })()
        } else {
            storage::prepare_data_directory(cli.data_dir.as_deref())
                .map_err(|e| e.to_string())
                .and_then(|data| {
                    storage::validate_data_directory(&data.root).map_err(|e| e.to_string())?;
                    serde_json::to_value(data).map_err(|e| e.to_string())
                })
        };
        let value = match &result {
            Ok(details) => serde_json::json!({"success":true,"game_input":false,"details":details}),
            Err(error) => serde_json::json!({"success":false,"game_input":false,"error":error}),
        };
        std::io::Write::write_all(
            &mut report_file,
            &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        report_file.sync_all().map_err(|e| e.to_string())?;
        return result.map(|_| ());
    }
    let explicit = cli.data_dir.is_some();
    let prepared =
        storage::prepare_data_directory(cli.data_dir.as_deref()).map_err(|e| e.to_string())?;
    let root = prepared.root;
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
                .set_title(format!("Welcome to {}", metin_desktop::PRODUCT_NAME))
                .set_description("Import profiles and settings from another data folder? Choose No to start fresh, or Cancel to leave without creating data.")
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show();
            match choice {
                rfd::MessageDialogResult::Yes => {
                    import = rfd::FileDialog::new()
                        .set_title("Select the previous application data folder")
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
    tracing::info!(build=%metin_desktop::build_info(),"Starting {}", metin_desktop::PRODUCT_NAME);
    if let Some(backup) = prepared.backup {
        tracing::info!(backup=%backup.display(), "Previous data migrated to Companion; complete backup retained");
    }
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title(metin_desktop::PRODUCT_NAME)
        .with_app_id(metin_desktop::APP_ID)
        .with_inner_size([460., 620.])
        .with_min_inner_size([400., 480.])
        .with_decorations(false)
        .with_resizable(true);
    #[cfg(feature = "ui-smoke")]
    let viewport = if cli.ui_icon_sheet {
        viewport.with_inner_size([460., 830.])
    } else if cli.ui_small {
        viewport.with_inner_size([400., 500.])
    } else {
        viewport
    };
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        metin_desktop::PRODUCT_NAME,
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
                    if cli.ui_fixture.is_some() {
                        app.set_window_picker_fixture(
                            metin_desktop::ui::WindowPickerFixture::Selected,
                        );
                    }
                    app.prepare_capture_fixture(cli.ui_buff_capture, cli.ui_sizes)
                        .map_err(std::io::Error::other)?;
                    if let Some(case) = cli.ui_window_picker {
                        app.set_window_picker_fixture(case);
                    }
                    app.prepare_actions_fixture(cli.ui_actions, cli.ui_run_state);
                    if let Some(enabled) = cli.ui_overlay_controls {
                        app.prepare_overlay_controls_fixture(enabled);
                    }
                    if cli.ui_buff_detail {
                        app.prepare_buff_detail_fixture()
                            .map_err(std::io::Error::other)?;
                    }
                    if cli.ui_buff_icon_picker {
                        cc.egui_ctx.data_mut(|d| {
                            d.insert_temp(
                                eframe::egui::Id::unique("buff-icon-picker-fixture"),
                                true,
                            )
                        });
                    }
                    if cli.ui_size_review {
                        app.prepare_size_review_fixture()
                            .map_err(std::io::Error::other)?;
                    }
                    if let Some(page) = cli.ui_settings {
                        app.prepare_settings_fixture(page);
                    }
                    if let Some(state) = cli.ui_updates.as_deref() {
                        app.prepare_updates_fixture(state)
                            .map_err(std::io::Error::other)?;
                    }
                    if let Some(page) = cli.ui_safety {
                        app.prepare_safety_fixture(page);
                    }
                    if cli.ui_safety_capture {
                        app.prepare_safety_capture_fixture()
                            .map_err(std::io::Error::other)?;
                    }
                    if cli.ui_icon_sheet {
                        app.prepare_icon_sheet();
                    }
                    if cli.ui_profile_menu {
                        app.prepare_profile_menu_fixture();
                    }
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
    let headless = cli.self_test || cli.input_driver.is_some() || cli.migration_report.is_some();
    if let Err(error) = startup(cli) {
        eprintln!("{}: {error}", metin_desktop::PRODUCT_NAME);
        if !headless {
            rfd::MessageDialog::new()
                .set_title(format!("Unable to start {}", metin_desktop::PRODUCT_NAME))
                .set_description(&error)
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
        std::process::exit(1);
    }
}
