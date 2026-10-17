use crate::{Result, ValidationError, require};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Space,
    Letter(char),
    Digit(u8),
    Function(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

impl Shortcut {
    pub fn parse(text: &str) -> Result<Self> {
        let parts: Vec<String> = text
            .split('+')
            .map(|v| v.trim().to_ascii_uppercase())
            .collect();
        require(
            !parts.is_empty() && parts.iter().all(|s| !s.is_empty()),
            "Invalid shortcut.",
        )?;
        let mut result = Self {
            ctrl: false,
            shift: false,
            alt: false,
            key: Key::Digit(0),
        };
        for part in &parts[..parts.len() - 1] {
            let slot = match part.as_str() {
                "CTRL" => &mut result.ctrl,
                "SHIFT" => &mut result.shift,
                "ALT" => &mut result.alt,
                _ => {
                    return Err(ValidationError(
                        "Allowed modifiers: Ctrl, Shift, Alt.".into(),
                    ));
                }
            };
            require(!*slot, "Duplicate shortcut modifier.")?;
            *slot = true;
        }
        let key = parts.last().expect("nonempty");
        result.key = if key == "SPACE" {
            Key::Space
        } else if key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase() {
            Key::Letter(key.chars().next().expect("one character"))
        } else if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() {
            Key::Digit(key.as_bytes()[0] - b'0')
        } else if let Some(n) = key
            .strip_prefix('F')
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|n| (1..=12).contains(n))
        {
            require(n != 7 && n != 8, "F7 and F8 are reserved for bot control.")?;
            Key::Function(n)
        } else {
            return Err(ValidationError(
                "Use Space, a letter, digit or F1–F12.".into(),
            ));
        };
        Ok(result)
    }
}
impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.shift {
            write!(f, "Shift+")?;
        }
        if self.alt {
            write!(f, "Alt+")?;
        }
        match self.key {
            Key::Space => write!(f, "Space"),
            Key::Letter(c) => write!(f, "{c}"),
            Key::Digit(n) => write!(f, "{n}"),
            Key::Function(n) => write!(f, "F{n}"),
        }
    }
}
