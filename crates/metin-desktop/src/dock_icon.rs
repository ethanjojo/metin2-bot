//! AppKit owns the Dock image; egui's window icon command does not update it.
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;

fn image(dark: bool) -> Result<Retained<NSImage>, String> {
    let data = NSData::with_bytes(crate::app_icon_png(dark));
    NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "AppKit could not decode the application icon".into())
}

pub fn set(dark: bool) -> Result<(), String> {
    let main = MainThreadMarker::new().ok_or("Dock icons must update on the main thread")?;
    let image = image(dark)?;
    // SAFETY: a non-null NSImage is supplied, on the AppKit main thread.
    unsafe { NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image)) };
    Ok(())
}

/// Used by the native CI probe to read back the actual AppKit property.
#[cfg(feature = "ui-smoke")]
pub fn matches(dark: bool) -> Result<bool, String> {
    let main = MainThreadMarker::new().ok_or("Dock verification requires the main thread")?;
    let expected = image(dark)?
        .TIFFRepresentation()
        .ok_or("Missing expected TIFF")?;
    let actual = NSApplication::sharedApplication(main)
        .applicationIconImage()
        .ok_or("Missing Dock image")?
        .TIFFRepresentation()
        .ok_or("Missing Dock TIFF")?;
    // AppKit can rewrite TIFF metadata and caches when assigning the Dock image.
    // Compare decoded samples, while still reading the actual application property.
    let expected = objc2_app_kit::NSBitmapImageRep::imageRepWithData(&expected)
        .ok_or("Cannot decode expected Dock bitmap")?;
    let actual = objc2_app_kit::NSBitmapImageRep::imageRepWithData(&actual)
        .ok_or("Cannot decode actual Dock bitmap")?;
    let layout = |bitmap: &objc2_app_kit::NSBitmapImageRep| {
        (
            bitmap.pixelsWide(),
            bitmap.pixelsHigh(),
            bitmap.samplesPerPixel(),
            bitmap.bitsPerSample(),
        )
    };
    let (width, height, samples, _) = layout(&expected);
    if layout(&actual) != layout(&expected) {
        return Err(format!(
            "Dock bitmap layout differs: {:?} vs {:?}",
            layout(&actual),
            layout(&expected)
        ));
    }
    if !(1..=16).contains(&samples) || width <= 0 || height <= 0 {
        return Err("Unsupported Dock bitmap layout".into());
    }
    for y in 0..height {
        for x in 0..width {
            let mut left = [0usize; 16];
            let mut right = [0usize; 16];
            // SAFETY: coordinates are within both bitmaps, and each output buffer
            // has at least samplesPerPixel entries, as checked above.
            unsafe {
                expected.getPixel_atX_y(std::ptr::NonNull::from(&mut left[0]), x, y);
                actual.getPixel_atX_y(std::ptr::NonNull::from(&mut right[0]), x, y);
            }
            if left[..samples as usize] != right[..samples as usize] {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
