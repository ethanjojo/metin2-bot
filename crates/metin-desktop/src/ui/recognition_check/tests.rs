use super::*;

fn setup() -> (MetinApp, egui::Context, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = MetinApp::from_context(&ctx, dir.path().to_owned()).unwrap();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 200,
        created: 1,
    });
    app.window_chooser.open = false;
    app.startup_update_open = false;
    app.tab = Tab::Calibration;
    (app, ctx, dir)
}
fn ready(app: &mut MetinApp) {
    app.recognition_check.state = DialogState::Ready(Report {
        context: app
            .recognition_context(RecognitionSubject::HpPanel)
            .unwrap(),
        captured_at: Instant::now(),
        source_size: [640, 480],
        evidence: SnapshotEvidence::Hp {
            score: 0.95,
            threshold: 0.88,
            matched: true,
            bounds: [0, 0, 9, 9],
        },
        images: vec![],
    });
}
fn render(
    app: &mut MetinApp,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<egui::Event>,
    full_app: bool,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| {
            if full_app {
                app.render(ui);
            } else {
                app.recognition_check_dialog(ui.ctx());
            }
        },
    );
    output.textures_delta.clear();
    output
}
fn text_regions(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => labels.push((
                text.galley.job.text.clone(),
                egui::Rect::from_min_size(text.pos, text.galley.size()),
            )),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, labels)),
            _ => {}
        }
    }
    let mut labels = vec![];
    for shape in &output.shapes {
        collect(&shape.shape, &mut labels);
    }
    labels
}
#[test]
fn recognition_result_has_a_modal_close_action_instead_of_an_inline_report() {
    let (mut app, ctx, _dir) = setup();
    ready(&mut app);
    let mut labels = vec![];
    for _ in 0..4 {
        labels = text_regions(&render(
            &mut app,
            &ctx,
            egui::vec2(460., 620.),
            vec![],
            true,
        ));
    }
    assert!(
        labels.iter().any(|(label, _)| label == "Close"),
        "Recognition results must open a modal with a fixed Close action; painted {labels:?}"
    );
    assert!(labels.iter().any(|(label, _)| label == "Check again"));
    app.runtime.shutdown().unwrap();
}
#[test]
fn pending_close_discards_late_frames_and_edits_invalidate_pending_ready_and_failure() {
    let (mut app, ctx, _dir) = setup();
    let check = app
        .recognition_context(RecognitionSubject::HpPanel)
        .unwrap();
    let requested_at = Instant::now();
    app.recognition_check.state = DialogState::Pending(PendingCheck {
        context: check.clone(),
        after_frame: 1,
        requested_at,
    });
    app.close_recognition_check();
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: Some(check.key),
        revision: check.revision,
        frame_id: 2,
        captured_at: Some(Instant::now()),
        frame: Some(metin_platform::Frame::new(10, 10, vec![0; 300]).unwrap()),
        geometry: Some(metin_platform::TargetState {
            target: check.key.target,
            left: 0,
            top: 0,
            width: 10,
            height: 10,
            visible: true,
            foreground: false,
        }),
        ..Default::default()
    });
    app.poll_recognition_check(&ctx);
    assert!(
        !app.recognition_check.is_open(),
        "A late capture must never reopen a closed modal"
    );
    for state in 0..3 {
        let check = app
            .recognition_context(RecognitionSubject::HpPanel)
            .unwrap();
        match state {
            0 => {
                app.recognition_check.state = DialogState::Pending(PendingCheck {
                    context: check,
                    after_frame: 2,
                    requested_at: Instant::now(),
                })
            }
            1 => ready(&mut app),
            _ => {
                app.recognition_check.state = DialogState::Failure(CheckFailure {
                    context: check,
                    message: "capture failed".into(),
                })
            }
        }
        app.editor.profile.timing.search_timeout += 1.;
        app.editor.record();
        app.poll_recognition_check(&ctx);
        assert!(matches!(
            app.recognition_check.state,
            DialogState::Invalidated { .. }
        ));
        app.close_recognition_check();
        app.poll_recognition_check(&ctx);
        assert!(!app.recognition_check.is_open());
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn retry_opens_pending_immediately_and_preserves_profile_timers_and_live_evidence() {
    let (mut app, ctx, _dir) = setup();
    ready(&mut app);
    let profile = app.editor.profile.clone();
    let snapshot = app.snapshot.clone();
    let generation = app.runtime.stop_generation();
    app.begin_recognition_check(RecognitionSubject::HpPanel);
    assert!(app.recognition_check.is_open() && app.recognition_check.is_pending());
    let DialogState::Pending(pending) = &app.recognition_check.state else {
        unreachable!()
    };
    assert_eq!(pending.after_frame, snapshot.frame_id);
    assert_eq!(app.editor.profile, profile);
    assert!(Arc::ptr_eq(&snapshot, &app.snapshot));
    assert_eq!(app.runtime.stop_generation(), generation);
    app.close_recognition_check();
    app.poll_recognition_check(&ctx);
    assert!(!app.recognition_check.is_open());
    app.runtime.shutdown().unwrap();
}
#[test]
fn ready_dialog_blocks_start_and_consumes_f7_without_saving() {
    let (mut app, _ctx, _dir) = setup();
    ready(&mut app);
    let error = app.start_profile().unwrap_err();
    assert!(
        error.contains("current selection or edit"),
        "An open ready modal must block Start before profile validation: {error}"
    );
    let request = crate::runtime::StartRequest {
        id: 42,
        key: app.key().unwrap(),
        revision: app.revision,
        generation: app.runtime.stop_generation(),
        requested_at: Instant::now(),
    };
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        start_request: Some(request),
        ..Default::default()
    });
    app.consume_start_request();
    assert_eq!(app.last_start_request, 42);
    assert!(
        app.error
            .as_ref()
            .is_some_and(|error| error.contains("current selection or edit"))
    );
    assert!(app.store.list().unwrap().is_empty());
    assert!(app.recognition_check.is_open());
    app.runtime.shutdown().unwrap();
}
#[test]
fn dialog_owns_delete_undo_and_escape_without_mutating_the_form() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.color = Some(metin_core::ColorConfig::default());
    app.editor.record();
    app.mode = EditMode::Color;
    app.editing = true;
    ready(&mut app);
    let profile = app.editor.profile.clone();
    for _ in 0..3 {
        render(&mut app, &ctx, egui::vec2(460., 620.), vec![], true);
    }
    for key in [egui::Key::Delete, egui::Key::Z, egui::Key::Escape] {
        let modifiers = if key == egui::Key::Z {
            egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            }
        } else {
            Default::default()
        };
        render(
            &mut app,
            &ctx,
            egui::vec2(460., 620.),
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            true,
        );
        assert_eq!(
            app.editor.profile, profile,
            "Modal key {key:?} must not alter the form"
        );
        assert!(
            app.editing,
            "Escape closing the modal must leave the underlying editor alone"
        );
    }
    assert!(!app.recognition_check.is_open());
    app.runtime.shutdown().unwrap();
}
#[test]
fn modal_footer_fits_400_pixels_at_150_percent_and_nested_popup_owns_escape() {
    let (mut app, ctx, _dir) = setup();
    ready(&mut app);
    if let DialogState::Ready(report) = &mut app.recognition_check.state {
        report.images = [
            ImageRole::Current,
            ImageRole::ActiveReference,
            ImageRole::InactiveReference,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, role)| ReportImage {
            role,
            texture: forms::frame_texture(
                &ctx,
                format!("recognition-layout-test-{index}"),
                &patterned(16, 16, 19),
                egui::TextureOptions::NEAREST,
            ),
        })
        .collect();
    }
    ctx.set_zoom_factor(1.5);
    let size = egui::vec2(400. / 1.5, 480. / 1.5);
    let mut output = render(&mut app, &ctx, size, vec![], false);
    for _ in 0..4 {
        output = render(&mut app, &ctx, size, vec![], false);
    }
    for label in ["Check again", "Close"] {
        let (_, rect) = text_regions(&output)
            .into_iter()
            .find(|(text, _)| text == label)
            .expect("Fixed modal action");
        assert!(
            ctx.content_rect().contains_rect(rect),
            "{label} outside scaled viewport: {rect:?}; modal {:?}; all {:?}",
            ctx.memory(|memory| memory.area_rect(egui::Id::unique("recognition-check-dialog"))),
            text_regions(&output),
        );
    }
    let popup = egui::Id::unique("recognition-nested-test-popup");
    egui::Popup::open_id(&ctx, popup);
    render(
        &mut app,
        &ctx,
        size,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        false,
    );
    assert!(
        app.recognition_check.is_open(),
        "Nested popup Escape must not close the modal"
    );
    egui::Popup::close_id(&ctx, popup);
    app.runtime.shutdown().unwrap();
}
#[test]
fn waiting_capture_failure_and_invalidated_states_have_explicit_distinct_explanations() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    let check = app
        .recognition_context(RecognitionSubject::HpPanel)
        .unwrap();
    app.recognition_check.state = DialogState::Pending(PendingCheck {
        context: check.clone(),
        after_frame: 0,
        requested_at: Instant::now(),
    });
    for _ in 0..3 {
        render(&mut app, &ctx, egui::vec2(460., 620.), vec![], false);
    }
    let waiting = text_regions(&render(
        &mut app,
        &ctx,
        egui::vec2(460., 620.),
        vec![],
        false,
    ));
    assert!(
        waiting
            .iter()
            .any(|(text, _)| text == "Waiting for a fresh image…")
    );
    assert!(
        waiting
            .iter()
            .any(|(text, _)| text == "Using unsaved settings")
    );
    app.recognition_check.state = DialogState::Failure(CheckFailure {
        context: check,
        message: "Capture stopped responding".into(),
    });
    let failure = text_regions(&render(
        &mut app,
        &ctx,
        egui::vec2(460., 620.),
        vec![],
        false,
    ));
    assert!(
        failure
            .iter()
            .any(|(text, _)| text == "Unable to check recognition")
    );
    assert!(!failure.iter().any(|(text, _)| text.contains("Match score")));
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    let invalidated = text_regions(&render(
        &mut app,
        &ctx,
        egui::vec2(460., 620.),
        vec![],
        false,
    ));
    assert!(
        invalidated
            .iter()
            .any(|(text, _)| text == "Result no longer applies")
    );
    assert!(!invalidated.iter().any(|(text, _)| text == "Edit reference"));
    app.runtime.shutdown().unwrap();
}
fn patterned(width: u32, height: u32, seed: usize) -> metin_platform::Frame {
    metin_platform::Frame::new(
        width,
        height,
        (0..(width * height * 3) as usize)
            .map(|index| ((index * 73 + (index / 7) * seed + seed) % 251) as u8)
            .collect(),
    )
    .unwrap()
}
#[test]
fn hp_scores_are_typed_and_a_negative_match_is_a_result_not_a_capture_error() {
    let frame = patterned(20, 20, 19);
    let template = metin_vision::crop(
        &frame,
        metin_core::Region {
            left: 0.,
            top: 0.,
            right: 0.4,
            bottom: 0.4,
        },
    )
    .unwrap();
    let mut profile = Profile::create("HP test");
    profile.frame_size = Some([20, 20]);
    profile.panel.threshold = 0.999;
    let positive = evaluate(
        &frame,
        &profile,
        Some(&template),
        RecognitionSubject::HpPanel,
    )
    .unwrap();
    assert!(
        matches!(positive.evidence, SnapshotEvidence::Hp { matched: true, score, threshold, .. } if score >= threshold)
    );
    let negative = evaluate(
        &patterned(20, 20, 113),
        &profile,
        Some(&template),
        RecognitionSubject::HpPanel,
    )
    .unwrap();
    assert!(
        matches!(negative.evidence, SnapshotEvidence::Hp { matched: false, score, threshold, .. } if score < threshold)
    );
}
#[test]
fn buff_reports_use_raw_match_flags_and_optional_scores_without_advancing_hold_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/vision");
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected.json")).unwrap())
            .unwrap();
    let case = &data["detectors"][5];
    let mut profile = Profile::from_json(&case["profile"].to_string()).unwrap();
    let frame = |index: usize| {
        metin_vision::load_image(
            &root
                .join("frames")
                .join(case["events"][index]["frame"].as_str().unwrap()),
        )
        .unwrap()
    };
    let active = frame(0);
    let inactive = frame(1);
    profile.frame_size = Some([active.width, active.height]);
    let original = profile.clone();
    for (frame, expected) in [
        (&active, metin_core::Presence::Active),
        (&inactive, metin_core::Presence::Absent),
    ] {
        let result = evaluate(frame, &profile, None, RecognitionSubject::Buff(0)).unwrap();
        assert!(
            matches!(result.evidence, SnapshotEvidence::Buff { presence, .. } if presence == expected)
        );
    }
    assert_eq!(profile, original);
    profile.buffs.skills[0].inactive_icon = profile.buffs.skills[0].icon.clone();
    let ambiguous = evaluate(&active, &profile, None, RecognitionSubject::Buff(0)).unwrap();
    assert!(
        matches!(ambiguous.evidence, SnapshotEvidence::Buff { presence: metin_core::Presence::Unknown, active_score: Some(a), inactive_score: Some(b), .. } if (a-b).abs() < 0.001)
    );
    profile.buffs.skills[0].inactive_icon = None;
    let optional = evaluate(&inactive, &profile, None, RecognitionSubject::Buff(0)).unwrap();
    assert!(matches!(
        optional.evidence,
        SnapshotEvidence::Buff {
            presence: metin_core::Presence::Unknown,
            inactive_score: None,
            ..
        }
    ));
}
#[cfg(feature = "ui-smoke")]
#[test]
fn named_buff_fixtures_produce_actual_active_inactive_and_ambiguous_evidence() {
    use base64::Engine;
    let (mut app, ctx, _dir) = setup();
    app.editor.freeze(patterned(100, 100, 19)).unwrap();
    let area = metin_core::Region {
        left: 0.,
        top: 0.,
        right: 0.8,
        bottom: 0.8,
    };
    let original = metin_core::BuffIcon {
        png: base64::engine::general_purpose::STANDARD
            .encode(metin_vision::encode_png(&patterned(45, 45, 31)).unwrap()),
        region: Some(area),
    };
    app.editor.profile.buffs.icon_region = Some(area);
    app.editor.profile.buffs.skills = vec![metin_core::BuffSkill {
        mode: metin_core::BuffMode::Inactive,
        interval_seconds: None,
        icon: Some(original.clone()),
        inactive_icon: Some(original),
        ..Default::default()
    }];
    let profile = app.editor.profile.clone();
    for (fixture, expected) in [
        (
            RecognitionCheckFixture::BuffActive,
            metin_core::Presence::Active,
        ),
        (
            RecognitionCheckFixture::BuffInactive,
            metin_core::Presence::Absent,
        ),
        (
            RecognitionCheckFixture::BuffAmbiguous,
            metin_core::Presence::Unknown,
        ),
    ] {
        app.prepare_recognition_check_state_fixture(&ctx, fixture)
            .unwrap();
        let DialogState::Ready(report) = &app.recognition_check.state else {
            panic!("The named result fixture must evaluate a frame");
        };
        assert!(
            matches!(report.evidence, SnapshotEvidence::Buff {presence, ..} if presence == expected),
            "Fixture {fixture:?} must produce its named raw state; got {:?}",
            report.evidence
        );
        assert!(
            report
                .images
                .iter()
                .filter(|image| matches!(
                    image.role,
                    ImageRole::ActiveReference | ImageRole::InactiveReference
                ))
                .all(|image| image.texture.size() == [45, 45])
        );
        assert_eq!(
            app.editor.profile, profile,
            "Fixture generation must only change local evaluation copies"
        );
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn fresh_check_rejects_previous_frames_stale_images_and_other_targets() {
    let target = TargetToken {
        handle: 1,
        pid: 2,
        created: 3,
    };
    let key = SessionKey {
        profile_id: uuid::Uuid::new_v4(),
        target,
    };
    let now = Instant::now();
    let mut snapshot = RuntimeSnapshot {
        key: Some(key),
        frame_id: 2,
        captured_at: Some(now),
        geometry: Some(metin_platform::TargetState {
            target,
            left: 0,
            top: 0,
            width: 10,
            height: 10,
            visible: true,
            foreground: false,
        }),
        frame: Some(metin_platform::Frame::new(10, 10, vec![0; 300]).unwrap()),
        ..Default::default()
    };
    assert!(fresh_check_frame(&snapshot, key, 0, 1, now - Duration::from_millis(1)).is_some());
    assert!(fresh_check_frame(&snapshot, key, 1, 1, now).is_none());
    assert!(fresh_check_frame(&snapshot, key, 0, 2, now).is_none());
    assert!(fresh_check_frame(&snapshot, key, 0, 1, now + Duration::from_millis(1)).is_none());
    assert!(
        fresh_check_frame(
            &snapshot,
            SessionKey {
                profile_id: uuid::Uuid::new_v4(),
                target
            },
            0,
            1,
            now
        )
        .is_none()
    );
    snapshot.captured_at = Some(now - Duration::from_secs(1));
    assert!(fresh_check_frame(&snapshot, key, 0, 1, now - Duration::from_secs(2)).is_none());
    snapshot.captured_at = Some(Instant::now() + Duration::from_secs(1));
    assert!(fresh_check_frame(&snapshot, key, 0, 1, now).is_none());
    snapshot.captured_at = Some(now);
    snapshot.geometry.as_mut().unwrap().target.created += 1;
    assert!(fresh_check_frame(&snapshot, key, 0, 1, now).is_none());
}
