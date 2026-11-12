//! The helper keeps its own window and runtime outside the files being replaced.
use super::{
    InstallProgress, process,
    session::{self, Handoff, Session},
};
use crate::{
    file_tree,
    ui::{skin, widgets},
};
use eframe::egui;
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug)]
pub enum HelperStatus {
    Verifying,
    Waiting,
    BackingUp,
    Installing(Option<InstallProgress>),
    Checking,
    Complete,
    Failed(String),
}
impl HelperStatus {
    fn text(&self) -> &str {
        match self {
            Self::Verifying => "Verifying the official update…",
            Self::Waiting => "Closing Companion…",
            Self::BackingUp => "Backing up your profiles and installation…",
            Self::Installing(Some(progress)) => match progress.phase {
                super::InstallPhase::Installing => "Installing files…",
                super::InstallPhase::Driver => "Checking the input driver…",
                super::InstallPhase::Finishing => "Finishing installation…",
            },
            Self::Installing(None) => "Starting installer…",
            Self::Checking => "Verifying the installed update…",
            Self::Complete => "Update complete",
            Self::Failed(_) => "Update could not finish",
        }
    }
    fn terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed(_))
    }
}

struct UpdateWindow {
    status: Arc<Mutex<HelperStatus>>,
    root: PathBuf,
    version: String,
    fixture: bool,
    last_progress: Option<InstallProgress>,
    session_id: uuid::Uuid,
}
impl UpdateWindow {
    fn should_close_after_abort(&self) -> bool {
        !self.fixture
            && session::read_json::<Handoff>(&self.root.join("abort.json"), 4096).is_ok_and(
                |record| {
                    record.schema == 1
                        && record.session == self.session_id
                        && record.message.is_none()
                },
            )
    }
}
impl eframe::App for UpdateWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.should_close_after_abort() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let mut status =
            self.status.lock().map(|s| s.clone()).unwrap_or_else(|_| {
                HelperStatus::Failed("The update worker is unavailable.".into())
            });
        if matches!(status, HelperStatus::Installing(_))
            && let Ok(bytes) = read_progress_bytes(&self.root.join("installer-progress.jsonl"))
            && let Ok(session) =
                session::read_json::<Session>(&self.root.join("session.json"), 65536)
            && let Ok(progress) = InstallProgress::decode(&bytes, session.id)
        {
            self.last_progress = Some(progress);
        }
        if matches!(status, HelperStatus::Installing(_)) {
            status = HelperStatus::Installing(self.last_progress.clone());
        }
        if ctx.input(|input| input.viewport().close_requested()) && !status.terminal() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        draw_window(ui, &status, &self.version, &self.root, self.fixture);
        if matches!(status, HelperStatus::Complete) && !self.fixture {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
fn draw_window(
    ui: &mut egui::Ui,
    status: &HelperStatus,
    version: &str,
    root: &Path,
    fixture: bool,
) {
    let ctx = ui.ctx().clone();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(10)))
        .show(ui, |ui| {
            skin::background(ui, false);
            if skin::simple_title(ui, "Companion") && status.terminal() && !fixture {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let body_height = (ui.available_height() - if status.terminal() { if ui.available_width() < 220. { 72. } else { 38. } } else { 0. }).max(1.);
            egui::ScrollArea::vertical().id_salt("update-helper-body")
                .auto_shrink([false,false]).max_height(body_height).show(ui, |ui| {
                ui.add_space((body_height * 0.12).min(70.));
                ui.vertical_centered(|ui| {
                    ui.heading("Updating Companion");
                    if !version.is_empty() { ui.weak(format!("Version {version}")); }
                    ui.add_space(24.);
                    ui.label(status.text());
                    ui.add_space(12.);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2((ui.available_width()-36.).max(1.),12.),egui::Sense::hover());
                    let fraction = match status {
                        HelperStatus::Complete => Some(1.),
                        HelperStatus::Installing(Some(progress)) => progress.fraction(),
                        _ => None,
                    };
                    widgets::progress_track(ui,rect,fraction);
                    if let Some(fraction)=fraction { ui.label(format!("{}%",(fraction*100.).floor() as u32)); }
                    else if !status.terminal() { ui.spinner(); }
                    ui.add_space(18.);
                    if let HelperStatus::Failed(error)=status {
                        ui.add(egui::Label::new(egui::RichText::new(error).color(widgets::RED)).wrap());
                        ui.add_space(10.);
                        ui.weak("Your backups and setup log remain available. No automation was started.");
                    } else { ui.weak("Your profiles and settings will be kept."); }
                    if fixture { ui.small("Preview only · no installation or elevation"); }
                });
            });
            if status.terminal() {
                ui.add_space(6.);
                ui.horizontal(|ui| {
                    if matches!(status,HelperStatus::Failed(_)) && widgets::plain_button(ui,"Open update log").clicked() && !fixture {
                        let _=process::spawn(&PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_default()).join("explorer.exe"),&[root.to_path_buf().into_os_string()]);
                    }
                    if widgets::plain_button(ui,"Close").clicked() && !fixture { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
            }
        });
}

#[cfg(feature = "ui-smoke")]
pub fn status_preview(ui: &mut egui::Ui, phase: &str) {
    let progress = |phase| InstallProgress {
        schema: 1,
        session: uuid::Uuid::nil(),
        phase,
        completed: 45,
        total: 100,
    };
    let status = match phase {
        "waiting" => HelperStatus::Waiting,
        "backup" => HelperStatus::BackingUp,
        "installing" => HelperStatus::Installing(Some(progress(super::InstallPhase::Installing))),
        "driver" => HelperStatus::Installing(Some(progress(super::InstallPhase::Driver))),
        "finishing" => HelperStatus::Installing(Some(progress(super::InstallPhase::Finishing))),
        "checking" => HelperStatus::Checking,
        "complete" => HelperStatus::Complete,
        "failed-long" => HelperStatus::Failed(
            "The update could not continue. Detailed recovery information follows. ".repeat(30),
        ),
        "failed" => HelperStatus::Failed(
            "The installer could not replace an application file. See setup.log for details."
                .into(),
        ),
        _ => HelperStatus::Verifying,
    };
    draw_window(ui, &status, "0.2.0", Path::new(""), true);
}

fn read_progress_bytes(path: &Path) -> Result<Vec<u8>, String> {
    const MAX_STREAM_BYTES: usize = 64 * 1024;
    let mut bytes = Vec::new();
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    options
        .open(path)
        .map_err(|e| e.to_string())?
        .take((MAX_STREAM_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_STREAM_BYTES {
        return Err("Installer progress exceeded its stream limit.".into());
    }
    let end = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .ok_or("No complete installer progress record yet.")?;
    let start = bytes[..end]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    if end - start > InstallProgress::MAX_RECORD_BYTES {
        return Err("Installer progress exceeded its record limit.".into());
    }
    Ok(bytes[start..end].to_vec())
}
fn set(status: &Mutex<HelperStatus>, value: HelperStatus) {
    if let Ok(mut state) = status.lock() {
        *state = value;
    }
}

fn backup_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let before = file_tree::fingerprints_except(source, None).map_err(|e| e.to_string())?;
    fs::create_dir(destination).map_err(|e| e.to_string())?;
    for relative in before.keys() {
        let output = destination.join(relative);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(source.join(relative), output).map_err(|e| e.to_string())?;
    }
    if file_tree::fingerprints_except(destination, None).map_err(|e| e.to_string())? != before {
        return Err("Backup verification failed; nothing was installed.".into());
    }
    Ok(())
}
/// Compare canonical identities, but preserve the registry's spelling for Setup.
/// Inno's existing-location preflight rejects Rust's Windows verbatim prefix.
fn validated_installation_directory(
    registered: PathBuf,
    expected: &Path,
) -> Result<PathBuf, String> {
    if registered.canonicalize().map_err(|e| e.to_string())? != expected {
        return Err(
            "The registered installation changed. Open the installed Companion and retry.".into(),
        );
    }
    Ok(registered)
}

fn install(
    root: &Path,
    session: &Session,
    status: &Mutex<HelperStatus>,
    _locks: &[File],
) -> Result<(), String> {
    let parent = process::Parent::open(session.parent_pid, session.parent_created)?;
    let registered = validated_installation_directory(
        process::registered_installation()?,
        &session.installation,
    )?;
    file_tree::reject_linked_ancestors(&root.join("installer-progress.jsonl"))
        .map_err(|e| e.to_string())?;
    session::write_new(
        &root.join("ready.json"),
        &Handoff {
            schema: 1,
            session: session.id,
            message: None,
        },
    )?;
    set(status, HelperStatus::Waiting);
    parent.wait()?;
    session::require_commit(root, session.id)?;
    let store_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(session.data_dir.join("profiles/.store.lock"))
        .map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&store_lock).map_err(
        |_| "Another Companion instance opened these profiles. Close it before updating.",
    )?;
    set(status, HelperStatus::BackingUp);
    let original_data =
        file_tree::fingerprints_except(&session.data_dir, None).map_err(|e| e.to_string())?;
    backup_tree(&session.data_dir, &root.join("data-backup"))?;
    backup_tree(&session.installation, &root.join("installation-backup"))?;
    session::require_commit(root, session.id)?;
    set(status, HelperStatus::Installing(None));
    let args: Vec<std::ffi::OsString> = vec![
        "/VERYSILENT".into(),
        "/SUPPRESSMSGBOXES".into(),
        "/NORESTART".into(),
        "/SP-".into(),
        "/ACCEPTINPUTDRIVER=1".into(),
        format!("/DIR={}", registered.display()).into(),
        format!(
            "/COMPANIONUPDATEPROGRESS={}",
            root.join("installer-progress.jsonl").display()
        )
        .into(),
        format!("/COMPANIONUPDATESESSION={}", session.id).into(),
        format!("/LOG={}", root.join("setup.log").display()).into(),
    ];
    let mut child = process::spawn(&root.join(session.candidate.asset_name()), &args)?;
    let exit = process::wait(&mut child, Duration::from_secs(900))?;
    if !exit.success() {
        return Err(format!(
            "The installer returned {}. See setup.log; the application was not restarted.",
            exit.code().unwrap_or(-1)
        ));
    }
    set(status, HelperStatus::Checking);
    crate::installation::verify_installed_bundle(&session.installation)?;
    let build: serde_json::Value =
        session::read_json(&session.installation.join("build-info.json"), 65536)?;
    if build.get("version").and_then(|v| v.as_str()) != Some(session.candidate.version()) {
        return Err(
            "The installed application version does not match the requested update.".into(),
        );
    }
    let app = session.installation.join("Companion.exe");
    let report = root.join("installed-self-test.json");
    let mut diagnostic = process::spawn(
        &app,
        &[
            "--self-test".into(),
            "--self-test-report".into(),
            report.clone().into_os_string(),
        ],
    )?;
    if !process::wait(&mut diagnostic, Duration::from_secs(60))?.success() {
        return Err("The installed application self-test failed. See the update logs.".into());
    }
    let test: serde_json::Value = session::read_json(&report, 1024 * 1024)?;
    if test.get("success").and_then(|v| v.as_bool()) != Some(true)
        || test.get("game_input").and_then(|v| v.as_bool()) != Some(false)
        || test.pointer("/build/version").and_then(|v| v.as_str())
            != Some(session.candidate.version())
    {
        return Err("The installed application did not pass its isolated self-test.".into());
    }
    let driver_report = root.join("installed-driver-status.json");
    let mut driver = process::spawn(
        &app,
        &[
            "--input-driver".into(),
            "status".into(),
            "--input-driver-report".into(),
            driver_report.clone().into_os_string(),
        ],
    )?;
    if !process::wait(&mut driver, Duration::from_secs(60))?.success() {
        return Err(
            "The input driver is not ready after update. See the retained driver status report."
                .into(),
        );
    }
    let driver_status: serde_json::Value = session::read_json(&driver_report, 1024 * 1024)?;
    if driver_status.get("success").and_then(|v| v.as_bool()) != Some(true)
        || driver_status.get("operation").and_then(|v| v.as_str()) != Some("status")
    {
        return Err("The input driver status could not be verified after update.".into());
    }
    if file_tree::fingerprints_except(&session.data_dir, None).map_err(|e| e.to_string())?
        != original_data
    {
        return Err(
            "Profile files changed during installation. The verified backup has been retained."
                .into(),
        );
    }
    drop(store_lock);
    let mut restarted = process::spawn(
        &app,
        &[
            "--data-dir".into(),
            session.data_dir.clone().into_os_string(),
        ],
    )?;
    let restart_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < restart_deadline {
        if let Some(exit) = restarted.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "The updated Companion closed during startup ({exit}). Your backups and logs were retained."
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    session::write_new(
        &root.join("complete.json"),
        &Handoff {
            schema: 1,
            session: session.id,
            message: None,
        },
    )?;
    set(status, HelperStatus::Complete);
    Ok(())
}

#[cfg(test)]
mod installation_directory_tests {
    use super::*;

    #[test]
    fn registry_path_keeps_setup_spelling_after_canonical_identity_validation() {
        let directory = tempfile::tempdir().unwrap();
        let registered = directory.path().join("Companion žluťoučký with spaces");
        fs::create_dir(&registered).unwrap();
        let canonical = registered.canonicalize().unwrap();
        let validated = validated_installation_directory(registered.clone(), &canonical).unwrap();
        assert_eq!(validated, registered);
        #[cfg(windows)]
        assert_ne!(
            validated, canonical,
            "Setup must not receive Rust's verbatim path prefix"
        );

        let changed = directory.path().join("Other installation");
        fs::create_dir(&changed).unwrap();
        assert!(validated_installation_directory(changed, &canonical).is_err());
    }
}

pub fn run(path: PathBuf) -> Result<(), String> {
    super::security::restrict_dll_search()?;
    let root = path
        .parent()
        .ok_or("Missing update session directory.")?
        .to_path_buf();
    let preview: Session = session::read_json(&path, 65536)?;
    let version = preview.candidate.version().to_owned();
    let bounds = preview.bounds.filter(|b| b.valid());
    let status = Arc::new(Mutex::new(HelperStatus::Verifying));
    let worker = Arc::clone(&status);
    let worker_root = root.clone();
    let viewport = egui::ViewportBuilder::default()
        .with_title(crate::PRODUCT_NAME)
        .with_app_id(crate::APP_ID)
        .with_inner_size([460., 620.])
        .with_min_inner_size([400., 480.])
        .with_decorations(false);
    let viewport = if let Some(bounds) = bounds {
        viewport
            .with_position([bounds.x, bounds.y])
            .with_inner_size([bounds.width, bounds.height])
    } else {
        viewport
    };
    eframe::run_native(
        crate::PRODUCT_NAME,
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            widgets::theme(&cc.egui_ctx);
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Icon(Some(Arc::new(
                    crate::app_icon().map_err(std::io::Error::other)?,
                ))));
            if let Some(bounds) = bounds {
                cc.egui_ctx.set_zoom_factor(bounds.zoom);
            }
            #[cfg(windows)]
            if let Some(window) = cc.winit_window() {
                use winit::platform::windows::{CornerPreference, WindowExtWindows};
                window.set_border_color(None);
                window.set_corner_preference(CornerPreference::DoNotRound);
            }
            std::thread::Builder::new()
                .name("companion-update-install".into())
                .spawn(move || {
                    let result = session::load(&path).and_then(|(session, locks)| {
                        install(&worker_root, &session, &worker, &locks)
                    });
                    if let Err(error) = result {
                        let _ = session::write_new(
                            &worker_root.join("handoff-error.json"),
                            &Handoff {
                                schema: 1,
                                session: preview.id,
                                message: Some(error.clone()),
                            },
                        );
                        set(&worker, HelperStatus::Failed(error));
                    }
                })
                .map_err(std::io::Error::other)?;

            Ok(Box::new(UpdateWindow {
                status,
                root,
                version,
                fixture: false,
                last_progress: None,
                session_id: preview.id,
            }))
        }),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod aborted_window_tests {
    use super::*;

    #[test]
    fn progress_reader_uses_only_the_latest_complete_bounded_record() {
        use crate::update_helper::InstallPhase;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("installer-progress.jsonl");
        let id = uuid::Uuid::new_v4();
        let first = InstallProgress {
            schema: 1,
            session: id,
            phase: InstallPhase::Installing,
            completed: 1,
            total: 10,
        };
        let second = InstallProgress {
            completed: 2,
            ..first.clone()
        };
        let line_one = serde_json::to_vec(&first).unwrap();
        let line_two = serde_json::to_vec(&second).unwrap();
        let mut stream = line_one.clone();
        stream.push(b'\n');
        stream.extend_from_slice(&line_two[..line_two.len() / 2]);
        fs::write(&path, &stream).unwrap();
        assert_eq!(
            InstallProgress::decode(&read_progress_bytes(&path).unwrap(), id)
                .unwrap()
                .completed,
            1
        );
        stream.truncate(line_one.len() + 1);
        stream.extend_from_slice(&line_two);
        stream.push(b'\n');
        stream.extend_from_slice(b"unfinished");
        fs::write(&path, &stream).unwrap();
        assert_eq!(
            InstallProgress::decode(&read_progress_bytes(&path).unwrap(), id)
                .unwrap()
                .completed,
            2
        );
        fs::write(&path, b"unfinished").unwrap();
        assert!(read_progress_bytes(&path).is_err());
        fs::write(&path, vec![b'\n'; 65537]).unwrap();
        assert!(read_progress_bytes(&path).is_err());
    }

    #[test]
    fn helper_closes_only_for_its_valid_abort_and_never_closes_preview() {
        let root = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4();
        let mut window = UpdateWindow {
            status: Arc::new(Mutex::new(HelperStatus::Failed("Check failed".into()))),
            root: root.path().to_owned(),
            version: "0.2.0".into(),
            fixture: false,
            last_progress: None,
            session_id: id,
        };
        assert!(!window.should_close_after_abort());
        let abort = root.path().join("abort.json");
        for (schema, session, message) in [
            (2, id, None),
            (1, uuid::Uuid::new_v4(), None),
            (1, id, Some("Not an abort".into())),
        ] {
            fs::write(
                &abort,
                serde_json::to_vec(&Handoff {
                    schema,
                    session,
                    message,
                })
                .unwrap(),
            )
            .unwrap();
            assert!(!window.should_close_after_abort());
        }
        fs::write(&abort, vec![b' '; 4097]).unwrap();
        assert!(!window.should_close_after_abort());
        fs::write(
            &abort,
            serde_json::to_vec(&Handoff {
                schema: 1,
                session: id,
                message: None,
            })
            .unwrap(),
        )
        .unwrap();
        assert!(window.should_close_after_abort());
        window.fixture = true;
        assert!(!window.should_close_after_abort());
    }
}
