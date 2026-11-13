use eframe::egui;
/// Shared bevelled select; popup lifetime and keyboard focus are provided by egui.
pub struct ComboBox {
    id: egui::Id,
    width: Option<f32>,
    text: String,
}
impl ComboBox {
    pub fn from_id_salt(id: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id: egui::Id::unique(id),
            width: None,
            text: String::new(),
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn selected_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }
    pub fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let response = combo_trigger(
            ui,
            self.id,
            &self.text,
            self.width.unwrap_or(ui.spacing().combo_width),
        );
        let popup_id = egui::Popup::default_response_id(&response);
        let frame = super::skin::popup_frame(ui);
        let width = response
            .rect
            .width()
            .min(ui.ctx().content_rect().width() - 16.);
        let popup = egui::Popup::menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(width)
            .frame(frame)
            .show(|ui| {
                bound_popup_content(ui, width, frame);
                menu_scroll(ui, self.id, 260., contents)
            });
        if popup.is_some() {
            ui.ctx().accesskit_node_builder(popup_id, |node| {
                node.set_role(egui::accesskit::Role::ListBox)
            });
        }
        egui::InnerResponse {
            inner: popup.map(|p| p.inner),
            response,
        }
    }
}

/// Area::default_width is an initial estimate, not a bound for long choice labels.
pub(super) fn bound_popup_content(ui: &mut egui::Ui, width: f32, frame: egui::Frame) {
    let content_width = (width - frame.total_margin().sum().x).max(1.);
    ui.set_width(content_width);
    ui.set_max_width(content_width);
}

pub fn combo_trigger(ui: &mut egui::Ui, id: egui::Id, text: &str, width: f32) -> egui::Response {
    let (_, rect) = ui.allocate_space(egui::vec2(width.min(ui.available_width()).max(40.), 28.));
    let response = ui.interact(rect, ui.make_persistent_id(id), egui::Sense::click());
    super::skin::bevel(ui, rect, response.has_focus() || response.hovered());
    let color = ui.visuals().text_color();
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.into(),
        egui::FontId::proportional(14.),
        color,
    );
    job.wrap.max_width = (rect.width() - 32.).max(1.);
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(rect.left() + 8., rect.center().y - galley.size().y / 2.),
        galley,
        color,
    );
    super::skin::native_sprite(
        ui,
        "scroll_down",
        &response,
        egui::Rect::from_center_size(
            egui::pos2(rect.right() - 12., rect.center().y),
            egui::vec2(17., 17.),
        ),
    );
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::new(egui::Role::ComboBox);
        info.enabled = ui.is_enabled();
        info.current_text_value = Some(text.to_owned());
        info
    });
    response
}

/// Explicit acceptance keeps scrollbar clicks from dismissing a menu.
pub fn option<T: PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    candidate: T,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    let mut response = option_row(ui, *value == candidate, label);
    if response.clicked() {
        if *value != candidate {
            *value = candidate;
            response.mark_changed();
        }
        ui.close();
    }
    response
}
pub fn option_row(
    ui: &mut egui::Ui,
    selected: bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    option_row_with_size(ui, selected, label.into(), None)
}

/// Explicit row geometry keeps text and adjacent profile actions on one baseline.
pub fn sized_option_row(
    ui: &mut egui::Ui,
    selected: bool,
    label: impl Into<egui::WidgetText>,
    size: egui::Vec2,
) -> egui::Response {
    option_row_with_size(ui, selected, label.into(), Some(size))
}

fn option_row_with_size(
    ui: &mut egui::Ui,
    selected: bool,
    label: egui::WidgetText,
    size: Option<egui::Vec2>,
) -> egui::Response {
    let hint = label.text().to_owned();
    let button = egui::Button::selectable(selected, label).truncate();
    let response = match size {
        Some(size) => ui.add_sized(size, button),
        None => ui.add(button),
    };
    if response.gained_focus() {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::unique(("select-row", &hint)),
            (response.id, response.rect, ui.clip_rect()),
        )
    });
    response.on_hover_text(hint)
}

/// Original scroll art, with egui retaining the only persistent scroll state.
/// A one-frame offset command applies an arrow/drag without a shadow offset.
pub fn menu_scroll<R>(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    max_height: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let command_id = ui.make_persistent_id(("menu-scroll-command", &salt));
    let command = ui.ctx().data_mut(|d| d.remove_temp::<f32>(command_id));
    let width = ui.available_width();
    let overflow_id = ui.make_persistent_id(("menu-scroll-overflow", &salt));
    let reserved = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(overflow_id))
        .unwrap_or(true);
    let content_width = (width - if reserved { 20. } else { 0. }).max(40.);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 3.;
        let result = ui
            .allocate_ui_with_layout(
                egui::vec2(content_width, max_height),
                egui::Layout::top_down_justified(egui::Align::Min),
                |ui| {
                    ui.set_width(content_width);
                    let scroll = egui::ScrollArea::vertical()
                        .id_salt(&salt)
                        .max_height(max_height)
                        .auto_shrink([false, true])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                        );
                    let scroll = if let Some(offset) = command {
                        scroll.vertical_scroll_offset(offset)
                    } else {
                        scroll
                    };
                    scroll.show(ui, contents)
                },
            )
            .inner;
        let max_offset = (result.content_size.y - result.inner_rect.height()).max(0.);
        let overflowing = max_offset > 0.;
        if overflowing != reserved {
            ui.ctx()
                .data_mut(|data| data.insert_temp(overflow_id, overflowing));
            // This is measured layout only; egui retains the sole scroll offset.
            // Re-run before presenting so a newly absent/present gutter cannot jump.
            ui.ctx().request_discard("Menu scrollbar gutter changed");
        }
        if max_offset > 0. {
            let (bar, _) = ui.allocate_exact_size(
                egui::vec2(17., result.inner_rect.height()),
                egui::Sense::hover(),
            );
            let up = egui::Rect::from_min_size(bar.min, egui::vec2(17., 17.));
            let down = egui::Rect::from_min_size(
                bar.left_bottom() - egui::vec2(0., 17.),
                egui::vec2(17., 17.),
            );
            let track = egui::Rect::from_min_max(up.left_bottom(), down.right_top());
            let thumb_height = (track.height() * result.inner_rect.height()
                / result.content_size.y)
                .clamp(18., track.height().max(18.));
            let travel = (track.height() - thumb_height).max(0.);
            let offset = result.state.clamped_offset().y;
            let thumb = egui::Rect::from_min_size(
                track.min + egui::vec2(0., travel * offset / max_offset),
                egui::vec2(17., thumb_height),
            );
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    egui::Id::unique("select-scroll-geometry"),
                    (up, down, thumb, offset, result.id),
                )
            });
            let mut next = offset;
            for (key, rect, style, step) in [
                ("up", up, "scroll_up", -28.),
                ("down", down, "scroll_down", 28.),
            ] {
                let response = ui.interact(rect, result.id.with(key), egui::Sense::click());
                super::skin::native_sprite(ui, style, &response, rect);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        if step < 0. {
                            "Scroll up"
                        } else {
                            "Scroll down"
                        },
                    )
                });
                if response.clicked() {
                    next += step;
                }
            }
            ui.painter()
                .rect_filled(track, 0., egui::Color32::from_gray(8));
            let response = ui.interact(
                thumb,
                result.id.with("thumb"),
                egui::Sense::click_and_drag(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::ScrollBar, true, "Options scroll position")
            });
            for (name, rect) in [
                (
                    "pattern/scrollbar_top",
                    egui::Rect::from_min_size(thumb.min, egui::vec2(17., 4.)),
                ),
                (
                    "pattern/scrollbar_middle",
                    egui::Rect::from_min_max(
                        thumb.min + egui::vec2(0., 4.),
                        thumb.max - egui::vec2(0., 4.),
                    ),
                ),
                (
                    "pattern/scrollbar_bottom",
                    egui::Rect::from_min_size(
                        thumb.left_bottom() - egui::vec2(0., 4.),
                        egui::vec2(17., 4.),
                    ),
                ),
            ] {
                super::skin::sprite(ui, name, rect);
            }
            if response.dragged() && travel > 0. {
                next += response.drag_delta().y * max_offset / travel;
            }
            let track_click = ui.interact(track, result.id.with("track"), egui::Sense::click());
            if track_click.clicked()
                && let Some(pos) = track_click.interact_pointer_pos()
                && !thumb.contains(pos)
            {
                next += if pos.y < thumb.top() {
                    -result.inner_rect.height()
                } else {
                    result.inner_rect.height()
                };
            }
            if (next - offset).abs() > 0.01 {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(command_id, next.clamp(0., max_offset)));
                ui.ctx().request_repaint();
            }
        }
        result.inner
    })
    .inner
}
