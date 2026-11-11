use crate::{PlatformError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidKey {
    Usage(u8),
    Modifier(u8),
}
/// Map Windows physical set-1 scan codes, after MapVirtualKeyEx layout mapping,
/// to USB HID usages. In particular Y/Z must follow the physical scan code.
pub fn scan_to_hid(scan: u32) -> Result<HidKey> {
    let modifier = match scan {
        0x1d => 1,
        0x2a => 2,
        0x38 => 4,
        0xe05b => 8,
        0xe01d => 16,
        0x36 => 32,
        0xe038 => 64,
        0xe05c => 128,
        _ => 0,
    };
    if modifier != 0 {
        return Ok(HidKey::Modifier(modifier));
    }
    let usage = match scan {
        0x01 => 0x29,
        0x02..=0x0b => (scan - 0x02 + 0x1e) as u8,
        0x0c => 0x2d,
        0x0d => 0x2e,
        0x0e => 0x2a,
        0x0f => 0x2b,
        0x10 => 0x14,
        0x11 => 0x1a,
        0x12 => 0x08,
        0x13 => 0x15,
        0x14 => 0x17,
        0x15 => 0x1c,
        0x16 => 0x18,
        0x17 => 0x0c,
        0x18 => 0x12,
        0x19 => 0x13,
        0x1a => 0x2f,
        0x1b => 0x30,
        0x1c => 0x28,
        0x1e => 0x04,
        0x1f => 0x16,
        0x20 => 0x07,
        0x21 => 0x09,
        0x22 => 0x0a,
        0x23 => 0x0b,
        0x24 => 0x0d,
        0x25 => 0x0e,
        0x26 => 0x0f,
        0x27 => 0x33,
        0x28 => 0x34,
        0x29 => 0x35,
        0x2b => 0x31,
        0x2c => 0x1d,
        0x2d => 0x1b,
        0x2e => 0x06,
        0x2f => 0x19,
        0x30 => 0x05,
        0x31 => 0x11,
        0x32 => 0x10,
        0x33 => 0x36,
        0x34 => 0x37,
        0x35 => 0x38,
        0x39 => 0x2c,
        0x3a => 0x39,
        0x3b..=0x44 => (scan - 0x3b + 0x3a) as u8,
        0x45 => 0x53,
        0x46 => 0x47,
        0x56 => 0x64,
        0x57 => 0x44,
        0x58 => 0x45,
        0xe01c => 0x58,
        0xe035 => 0x54,
        0xe047 => 0x4a,
        0xe048 => 0x52,
        0xe049 => 0x4b,
        0xe04b => 0x50,
        0xe04d => 0x4f,
        0xe04f => 0x4d,
        0xe050 => 0x51,
        0xe051 => 0x4e,
        0xe052 => 0x49,
        0xe053 => 0x4c,
        _ => {
            return Err(PlatformError(
                "Unsupported physical key for virtual HID input.".into(),
            ));
        }
    };
    Ok(HidKey::Usage(usage))
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HidKeyboard {
    pub modifiers: u8,
    pub keys: Vec<u8>,
}
impl HidKeyboard {
    pub fn set(&mut self, key: HidKey, down: bool) -> Result<()> {
        match key {
            HidKey::Modifier(bit) if bit.is_power_of_two() => {
                if down {
                    self.modifiers |= bit;
                } else {
                    self.modifiers &= !bit;
                }
            }
            HidKey::Usage(usage) if (4..0xe0).contains(&usage) => {
                if down {
                    if !self.keys.contains(&usage) {
                        if self.keys.len() == 6 {
                            return Err(PlatformError(
                                "Virtual keyboard supports at most six held keys.".into(),
                            ));
                        }
                        self.keys.push(usage);
                    }
                } else {
                    self.keys.retain(|key| *key != usage);
                }
            }
            _ => return Err(PlatformError("Invalid HID key report.".into())),
        }
        Ok(())
    }
}
