//! Creates isolated demonstration data for real application screenshots.
use base64::{Engine, engine::general_purpose::STANDARD};
use metin_core::{BuffIcon, BuffMode, BuffSkill, ColorConfig, Profile, Region};
use metin_desktop::storage::{ProfileStore, UISettings};
use metin_platform::Frame;
use std::{fs, path::PathBuf};

fn rect(pixels: &mut [u8], bounds: [usize; 4], rgb: [u8; 3]) {
    let [x, y, width, height] = bounds;
    for row in y..y + height {
        for column in x..x + width {
            let offset = (row * 960 + column) * 3;
            pixels[offset..offset + 3].copy_from_slice(&[rgb[2], rgb[1], rgb[0]]);
        }
    }
}

fn region(bounds: [u32; 4]) -> Region {
    let [x, y, width, height] = bounds;
    Region {
        left: f64::from(x) / 960.,
        top: f64::from(y) / 600.,
        right: f64::from(x + width) / 960.,
        bottom: f64::from(y + height) / 600.,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Supply a fresh demo output folder")?,
    );
    if root.exists() {
        return Err("Demo output already exists; choose a fresh folder".into());
    }
    fs::create_dir_all(&root)?;
    let mut pixels = vec![0; 960 * 600 * 3];
    rect(&mut pixels, [0, 0, 960, 600], [39, 49, 49]);
    // A geometric sample scene, not a screenshot of a running client.
    for (x, y, width, height) in [(80, 150, 200, 55), (390, 220, 290, 38), (180, 420, 460, 42)] {
        rect(&mut pixels, [x, y, width, height], [46, 59, 56]);
    }
    for (x, y) in [(250, 245), (470, 345), (680, 235), (440, 155)] {
        for row in 0usize..84 {
            let half = 18 + (42usize.saturating_sub(row.abs_diff(42))) / 2;
            rect(&mut pixels, [x - half, y + row, half * 2, 1], [210, 63, 52]);
            rect(&mut pixels, [x, y + row, half / 2, 1], [235, 86, 65]);
        }
    }
    rect(&mut pixels, [335, 20, 290, 66], [22, 28, 32]);
    rect(&mut pixels, [385, 54, 215, 10], [163, 55, 56]);
    for y in 0..24 {
        for x in 0..24 {
            let color = if (x / 4 + y / 4) % 2 == 0 {
                [224, 194, 109]
            } else {
                [65, 79, 89]
            };
            rect(&mut pixels, [344 + x, 30 + y, 1, 1], color);
        }
    }
    for (x, color) in [(40, [57, 153, 133]), (100, [88, 97, 111])] {
        rect(&mut pixels, [x, 32, 44, 44], color);
        rect(&mut pixels, [x + 18, 38, 8, 32], [218, 229, 217]);
        rect(&mut pixels, [x + 10, 49, 24, 8], [218, 229, 217]);
    }
    rect(&mut pixels, [830, 35, 110, 120], [26, 35, 38]);
    rect(&mut pixels, [842, 47, 86, 96], [63, 83, 75]);
    rect(&mut pixels, [300, 530, 360, 48], [23, 30, 34]);
    for x in (312..648).step_by(48) {
        rect(&mut pixels, [x, 540, 30, 28], [67, 82, 91]);
    }
    let scene = Frame::new(960, 600, pixels)?;
    fs::write(root.join("scene.png"), metin_vision::encode_png(&scene)?)?;
    let icon = |bounds| -> Result<BuffIcon, Box<dyn std::error::Error>> {
        let crop = metin_vision::crop(&scene, region(bounds))?;
        Ok(BuffIcon {
            png: STANDARD.encode(metin_vision::encode_png(&crop)?),
            region: Some(region(bounds)),
        })
    };
    let mut profile = Profile::create("Main profile");
    profile.id = uuid::Uuid::from_u128(1);
    profile.frame_size = Some([960, 600]);
    profile.region = Some(region([70, 110, 730, 405]));
    profile.masks = vec![region([820, 25, 125, 140]), region([290, 525, 380, 60])];
    profile.color = Some(ColorConfig::default());
    profile.panel.region = Some(region([335, 20, 290, 66]));
    profile.template_region = Some(region([344, 30, 24, 24]));
    profile.buffs.enabled = true;
    profile.buffs.icon_region = Some(region([30, 22, 125, 65]));
    profile.buffs.skills = vec![
        BuffSkill {
            name: "Buff 1".into(),
            shortcut: "F1".into(),
            interval_seconds: Some(120.),
            ..Default::default()
        },
        BuffSkill {
            name: "Buff 2".into(),
            shortcut: "F2".into(),
            mode: BuffMode::Inactive,
            icon: Some(icon([40, 32, 44, 44])?),
            inactive_icon: Some(icon([100, 32, 44, 44])?),
            interval_seconds: None,
            ..Default::default()
        },
        BuffSkill {
            name: "Buff 3".into(),
            shortcut: "F3".into(),
            interval_seconds: Some(180.),
            ..Default::default()
        },
    ];
    let template = metin_vision::crop(&scene, profile.template_region.unwrap())?;
    let mut store = ProfileStore::open(&root.join("profiles"))?;
    store.save(&profile, Some(&template))?;
    store.save(&Profile::create("Secondary profile"), None)?;
    store.select(profile.id)?;
    UISettings::default().save(&root)?;
    println!("Demonstration profile created in {}", root.display());
    Ok(())
}
