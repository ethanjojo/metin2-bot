use eframe::icon_data::from_png_bytes;

#[test]
fn runtime_identity_and_transparent_icon_match_packaged_assets() {
    let identity: serde_json::Value =
        serde_json::from_str(include_str!("../../../packaging/app-identity.json")).unwrap();
    let build = metin_desktop::build_info();
    assert_eq!(build["publisher"], identity["publisher"]);
    assert_eq!(build["product"], identity["product_name"]);
    assert_eq!(metin_desktop::APP_ID, identity["app_id"]);
    let icon = metin_desktop::app_icon(false).unwrap();
    assert_eq!([icon.width, icon.height], [256, 256]);
    assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    assert_eq!(icon.rgba[3], 0);
    assert!(
        icon.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 200 && p[..3] == [0, 0, 0])
    );
}

#[test]
fn macos_icon_contains_all_expected_png_resolutions() {
    let icns = include_bytes!("../../../packaging/icons/metinbot.icns");
    assert_eq!(&icns[..4], b"icns");
    assert_eq!(
        u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize,
        icns.len()
    );
    let mut offset = 8;
    let mut sizes = Vec::new();
    while offset < icns.len() {
        let length = u32::from_be_bytes(icns[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let image = from_png_bytes(&icns[offset + 8..offset + length]).unwrap();
        assert_eq!(image.width, image.height);
        assert_eq!(image.rgba[3], 0);
        sizes.push(image.width);
        offset += length;
    }
    sizes.sort();
    assert_eq!(sizes, [16, 32, 64, 128, 256, 512, 1024]);
}

#[test]
fn dark_icon_is_white_with_exactly_the_same_alpha_as_the_black_light_icon() {
    let light = metin_desktop::app_icon(false).unwrap();
    let dark = metin_desktop::app_icon(true).unwrap();
    for (black, white) in light
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dark.rgba.as_chunks::<4>().0)
    {
        assert_eq!(black[3], white[3]);
        if black[3] > 0 {
            assert_eq!(&black[..3], &[0, 0, 0]);
            assert_eq!(&white[..3], &[255, 255, 255]);
        }
    }
}

#[test]
fn windows_icon_contains_valid_transparent_pngs_at_all_desktop_sizes() {
    let ico = include_bytes!("../../../packaging/icons/metinbot.ico");
    assert_eq!(&ico[..4], &[0, 0, 1, 0]);
    let count = u16::from_le_bytes(ico[4..6].try_into().unwrap()) as usize;
    let sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
    assert_eq!(count, sizes.len());
    for (i, size) in sizes.into_iter().enumerate() {
        let entry = 6 + i * 16;
        let length = u32::from_le_bytes(ico[entry + 8..entry + 12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(ico[entry + 12..entry + 16].try_into().unwrap()) as usize;
        let png = from_png_bytes(&ico[offset..offset + length]).unwrap();
        assert_eq!([png.width, png.height], [size, size]);
        assert_eq!(png.rgba[3], 0);
        assert!(png.rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
    }
}
