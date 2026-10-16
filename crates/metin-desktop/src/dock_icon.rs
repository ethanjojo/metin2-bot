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
    Ok(actual.isEqualToData(&expected))
}
