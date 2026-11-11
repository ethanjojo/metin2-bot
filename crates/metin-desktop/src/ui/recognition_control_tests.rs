use super::*;

fn render_controls(
    app: &mut MetinApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 480.),
            )),
            events,
            ..Default::default()
        },
        |root| {
            egui::CentralPanel::default().show(root, |ui| {
                app.overlay_controls(ui);
            });
        },
    );
    output.textures_delta.clear();
    output
}

fn click_control(app: &mut MetinApp, ctx: &egui::Context, label: &str) {
    render_controls(app, ctx, vec![]);
    let output = render_controls(app, ctx, vec![]);
    let point = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing recognition control {label}"))
        .1
        .center();
    for pressed in [true, false] {
        render_controls(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

#[test]
fn overlay_children_are_disabled_when_off_and_retain_independent_choices() {
    let (mut app, ctx, _dir) = setup();
    let original = app.editor.profile.clone();
    app.run_live = false;
    app.show_detections = true;
    app.show_regions = false;
    app.show_sizes = true;
    for label in ["Detected targets", "Calibration areas", "Sizes (px)"] {
        click_control(&mut app, &ctx, label);
    }
    assert!(app.show_detections && !app.show_regions && app.show_sizes);
    click_control(&mut app, &ctx, "Game overlay");
    assert!(app.run_live);
    click_control(&mut app, &ctx, "Calibration areas");
    assert!(app.show_detections && app.show_regions && app.show_sizes);
    click_control(&mut app, &ctx, "Detected targets");
    assert!(!app.show_detections && app.show_regions && app.show_sizes);
    click_control(&mut app, &ctx, "Game overlay");
    assert!(!app.run_live);
    assert!(!app.show_detections && app.show_regions && app.show_sizes);
    assert_eq!(
        app.editor.profile, original,
        "Presentation options cannot mutate recognition or automation"
    );
    app.runtime.shutdown().unwrap();
}
