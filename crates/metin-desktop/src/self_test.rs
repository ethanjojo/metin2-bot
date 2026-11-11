use eframe::egui;
use std::{collections::BTreeMap, path::Path};
pub fn run(report: &Path) -> Result<(), String> {
    let mut checks = BTreeMap::new();
    let mut errors = BTreeMap::new();
    let mut check = |name: &str, result: Result<(), String>| {
        checks.insert(name.to_owned(), result.is_ok());
        if let Err(error) = result {
            errors.insert(name.to_owned(), error);
        }
    };
    check(
        "vision",
        (|| {
            let pixels = (0..30 * 30)
                .flat_map(|n| {
                    [
                        (n % 251) as u8,
                        ((n * 7) % 253) as u8,
                        ((n * 13) % 249) as u8,
                    ]
                })
                .collect();
            let image = metin_platform::Frame::new(30, 30, pixels).map_err(|e| e.to_string())?;
            metin_vision::validate_template(&image).map_err(|e| e.to_string())?;
            let encoded = metin_vision::encode_png(&image).map_err(|e| e.to_string())?;
            let decoded = metin_vision::decode_png(&encoded).map_err(|e| e.to_string())?;
            if image != decoded {
                return Err("PNG roundtrip changed pixels.".into());
            }
            Ok(())
        })(),
    );
    check(
        "storage",
        (|| {
            let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
            let mut store =
                crate::storage::ProfileStore::open(dir.path()).map_err(|e| e.to_string())?;
            let p = metin_core::Profile::create("Self-test Žluťoučký");
            store.save(&p, None).map_err(|e| e.to_string())?;
            store.select(p.id).map_err(|e| e.to_string())?;
            if store.load(p.id).map_err(|e| e.to_string())?.0 != p {
                return Err("Profile roundtrip changed data.".into());
            }
            Ok(())
        })(),
    );
    {
        let context = egui::Context::default();
        crate::ui::widgets::theme(&context);
        let mut output = context.run_ui(egui::RawInput::default(), |root| {
            egui::CentralPanel::default().show(root, |ui| {
                ui.label(format!("{} Žluťoučký kůň", crate::PRODUCT_NAME));
                let _ = ui.button("Start");
            });
        });
        check(
            "metin_theme",
            if output.shapes.is_empty() {
                Err("GUI produced no shapes.".into())
            } else {
                Ok(())
            },
        );
        output.textures_delta.clear();
    }
    let success = checks.values().all(|v| *v);
    let value = serde_json::json!({"build":crate::build_info(),"success":success,"checks":checks,"errors":errors,"game_input":false,"gpu_window_tested":false});
    crate::storage::atomic_write(
        report,
        &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if success {
        Ok(())
    } else {
        Err("Self-test failed; see the JSON report.".into())
    }
}
