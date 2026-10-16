use super::*;
use crate::storage::WindowPreference;
use metin_platform::TargetInfo;

#[cfg(any(test, feature = "ui-smoke"))]
pub(super) struct WindowFixture {
    pub(super) snapshot: Arc<RuntimeSnapshot>,
    pub(super) open_popup: bool,
}
#[cfg(feature = "ui-smoke")]
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum WindowPickerFixture {
    All,
    Filtered,
    Empty,
    Ambiguous,
    Missing,
    Selected,
}

#[cfg(feature = "ui-smoke")]
impl MetinApp {
    /// Synthetic catalog for native rendering only; unavailable capabilities
    /// keep Start blocked. Excluded from standard release builds.
    pub fn set_window_picker_fixture(&mut self, case: WindowPickerFixture) {
        let mut targets=vec![
            TargetInfo {token:TargetToken {handle:100,pid:1201,created:1},title:"Forest · Warrior".into(),process_name:"server-a.exe".into()},
            TargetInfo {token:TargetToken {handle:200,pid:1202,created:2},title:"Dungeon · Sura".into(),process_name:"server-b.exe".into()},
            TargetInfo {token:TargetToken {handle:300,pid:1203,created:3},title:"A very long application title demonstrating bounded window rows and full hover help".into(),process_name:"other-client.exe".into()},
        ];
        self.settings
            .profile_windows
            .remove(&self.editor.profile.id);
        self.window_search = match case {
            WindowPickerFixture::Filtered => "SERVER-B".into(),
            WindowPickerFixture::Empty => "no-such-window".into(),
            _ => String::new(),
        };
        if matches!(case, WindowPickerFixture::Ambiguous) {
            targets[1].title = targets[0].title.clone();
            targets[1].process_name = targets[0].process_name.clone();
        }
        if matches!(
            case,
            WindowPickerFixture::Ambiguous | WindowPickerFixture::Missing
        ) {
            self.settings.profile_windows.insert(
                self.editor.profile.id,
                WindowPreference {
                    process_name: if matches!(case, WindowPickerFixture::Missing) {
                        "closed.exe".into()
                    } else {
                        targets[0].process_name.clone()
                    },
                    title: if matches!(case, WindowPickerFixture::Missing) {
                        "Closed game window".into()
                    } else {
                        targets[0].title.clone()
                    },
                },
            );
        }
        self.target = if matches!(case, WindowPickerFixture::Selected) {
            Some(targets[0].token)
        } else {
            None
        };
        self.target_list_revision = 0;
        self.tab = Tab::Calibration;
        self.window_fixture = Some(WindowFixture {
            snapshot: Arc::new(RuntimeSnapshot {
                targets,
                targets_revision: 1,
                ..Default::default()
            }),
            open_popup: !matches!(
                case,
                WindowPickerFixture::Missing | WindowPickerFixture::Selected
            ),
        });
    }
}

pub(super) fn matches_search(window: &TargetInfo, search: &str) -> bool {
    let search = search.trim().to_lowercase();
    search.is_empty()
        || window.title.to_lowercase().contains(&search)
        || window.process_name.to_lowercase().contains(&search)
        || window.token.pid.to_string() == search
}

pub(super) fn remembered_matches<'a>(
    windows: &'a [TargetInfo],
    preference: &WindowPreference,
) -> Vec<&'a TargetInfo> {
    windows
        .iter()
        .filter(|window| {
            window
                .process_name
                .eq_ignore_ascii_case(&preference.process_name)
                && window.title == preference.title
        })
        .collect()
}

impl MetinApp {
    pub(super) fn select_window(&mut self, target: TargetToken) -> Result<(), String> {
        let window = self
            .snapshot
            .targets
            .iter()
            .find(|w| w.token == target)
            .ok_or("This window is no longer listed. Refresh and select it again.")?;
        let preference = WindowPreference {
            process_name: window.process_name.clone(),
            title: window.title.clone(),
        };
        self.runtime.stop();
        if self.settings.profile_windows.get(&self.editor.profile.id) != Some(&preference) {
            let mut settings = self.settings.clone();
            settings
                .profile_windows
                .insert(self.editor.profile.id, preference);
            settings
                .save(&self.root)
                .map_err(|error| error.to_string())?;
            self.settings = settings;
        }
        self.target = Some(target);
        self.snapshot_requested = None;
        self.configure()
    }

    pub(super) fn reconcile_windows(&mut self) {
        if self.snapshot.targets_revision == 0
            || self.snapshot.targets_revision == self.target_list_revision
        {
            return;
        }
        self.target_list_revision = self.snapshot.targets_revision;
        if self
            .target
            .is_some_and(|target| !self.snapshot.targets.iter().any(|w| w.token == target))
        {
            self.runtime.stop();
            self.send(Command::Detach);
            self.target = None;
            self.snapshot_requested = None;
            self.live = false;
        }
        if self.target.is_none()
            && let Some(preference) = self.settings.profile_windows.get(&self.editor.profile.id)
        {
            let matches = remembered_matches(&self.snapshot.targets, preference);
            if matches.len() == 1 {
                let target = matches[0].token;
                if let Err(error) = self.select_window(target) {
                    self.error = Some(error);
                }
            }
        }
    }

    pub(super) fn window_choice(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Game window");
            crate::ui_controls::help(ui,"Choose the exact game window. This profile remembers its process and title. A single exact match is selected automatically; automation always requires Start.");
        });
        let selected = self
            .snapshot
            .targets
            .iter()
            .find(|w| Some(w.token) == self.target);
        let selected_text = selected
            .map(|w| format!("{} · {}", w.title, w.process_name))
            .unwrap_or_else(|| "Select a game window".into());
        let mut choice = None;
        let mut search_id = None;
        let width = ui.available_width().min(340.);
        let picker=egui::ComboBox::from_id_salt("game-window").truncate().width(width)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .height(300.).selected_text(&selected_text).show_ui(ui,|ui| {
                let popup_width=(ui.ctx().content_rect().width()-40.).clamp(250.,460.);
                ui.set_min_width(popup_width);
                ui.set_max_width(popup_width);
                ui.horizontal(|ui| {
                    let clear_width=if self.window_search.is_empty(){0.}else{52.};
                    let search=ui.add(egui::TextEdit::singleline(&mut self.window_search)
                        .hint_text("Search title or process…").desired_width((ui.available_width()-clear_width).max(80.)))
                        .on_hover_text("Search open windows by title, process filename or exact PID. Search is temporary and never saved.");
                    search_id=Some(search.id);
                    if !self.window_search.is_empty()
                        && ui.small_button("Clear").on_hover_text("Clear the temporary search.").clicked() {self.window_search.clear();search.request_focus();}
                });
                ui.separator();
                let windows=self.snapshot.targets.iter().filter(|w|matches_search(w,&self.window_search)).collect::<Vec<_>>();
                if windows.is_empty() {
                    ui.label(if self.snapshot.targets.is_empty(){"No open windows available. Refresh after opening the client."}else{"No windows match this search."});
                }
                for window in windows {
                    let label=format!("{} · {} · PID {}",window.title,window.process_name,window.token.pid);
                    if ui.add(egui::Button::selectable(self.target==Some(window.token),&label).truncate())
                        .on_hover_text(&label).clicked() {
                        choice=Some(window.token);
                        ui.close();
                    }
                }
            }).response;
        if picker.clicked()
            && let Some(id) = search_id
        {
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        #[cfg(any(test, feature = "ui-smoke"))]
        if self
            .window_fixture
            .as_ref()
            .is_some_and(|fixture| fixture.open_popup)
            && !egui::ComboBox::is_open(ui.ctx(), picker.id)
        {
            egui::Popup::open_id(ui.ctx(), picker.id.with("popup"));
        }
        picker.on_hover_text(
            selected
                .map(|w| format!("{} · {} · PID {}", w.title, w.process_name, w.token.pid))
                .unwrap_or_else(|| {
                    "Choose an open game window. All listed processes remain available.".into()
                }),
        );
        if let Some(target) = choice
            && let Err(error) = self.select_window(target)
        {
            self.error = Some(error);
        }
        if self.target.is_none() {
            if !self.snapshot.capabilities.capture && !self.snapshot.capabilities.reason.is_empty()
            {
                ui.small("Window capture is unavailable on this platform.")
                    .on_hover_text(&self.snapshot.capabilities.reason);
            } else if self.snapshot.targets_revision == 0 {
                ui.small("Checking open windows…");
            } else if let Some(preference) =
                self.settings.profile_windows.get(&self.editor.profile.id)
            {
                let count = remembered_matches(&self.snapshot.targets, preference).len();
                ui.small(if count > 1 {
                    "Several windows match this profile. Select one and check its PID."
                } else {
                    "Remembered window not found. Select another window or Refresh."
                })
                .on_hover_text(format!(
                    "Remembered for this profile: {} · {}",
                    preference.title, preference.process_name
                ));
            } else if self.snapshot.targets.is_empty() {
                ui.small("No open windows. Open the client and Refresh.");
            } else {
                ui.small("Select a window. This profile will remember your choice.");
            }
        }
    }
}
