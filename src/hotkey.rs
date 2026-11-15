use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HotkeyAction {
    TogglePause,
    EndSession,
    ToggleOverlay,
    ToggleDashboard,
}

impl HotkeyAction {
    pub const ALL: [HotkeyAction; 4] = [
        HotkeyAction::TogglePause,
        HotkeyAction::EndSession,
        HotkeyAction::ToggleOverlay,
        HotkeyAction::ToggleDashboard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HotkeyAction::TogglePause => "Pause / resume tracking",
            HotkeyAction::EndSession => "End current session",
            HotkeyAction::ToggleOverlay => "Show / hide overlay",
            HotkeyAction::ToggleDashboard => "Open / close dashboard",
        }
    }
}

/// A key combination, matched against the low-level keyboard hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub vk: u16,
}

const NAMED_KEYS: &[(&str, u16)] = &[
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Tab", 0x09),
    ("Escape", 0x1B),
    ("Esc", 0x1B),
    ("Backspace", 0x08),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("Up", 0x26),
    ("Down", 0x28),
    ("Left", 0x25),
    ("Right", 0x27),
    ("Pause", 0x13),
    ("ScrollLock", 0x91),
    ("PrintScreen", 0x2C),
];

fn parse_key(part: &str) -> Option<u16> {
    let upper = part.to_ascii_uppercase();
    if upper.len() == 1 {
        let c = upper.as_bytes()[0];
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as u16);
        }
    }
    if let Some(n) = upper.strip_prefix('F') {
        if let Ok(n) = n.parse::<u16>() {
            if (1..=24).contains(&n) {
                return Some(0x6F + n);
            }
        }
    }
    NAMED_KEYS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(part))
        .map(|(_, vk)| *vk)
}

fn key_name(vk: u16) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => NAMED_KEYS
            .iter()
            .find(|(_, v)| *v == vk)
            .map(|(n, _)| n.to_string())
            .unwrap_or_else(|| format!("0x{vk:02X}")),
    }
}

impl Hotkey {
    /// Parses strings like "Ctrl+Alt+P" or "F9".
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut hk = Hotkey::default();
        let mut key = None;
        for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => hk.ctrl = true,
                "alt" => hk.alt = true,
                "shift" => hk.shift = true,
                "win" | "super" | "meta" => hk.win = true,
                _ => {
                    if key.is_some() {
                        return Err("Only one non-modifier key is allowed".into());
                    }
                    key = Some(parse_key(part).ok_or_else(|| format!("Unknown key '{part}'"))?);
                }
            }
        }
        hk.vk = key.ok_or("Missing a key, e.g. Ctrl+Alt+P")?;
        let standalone_ok = (0x70..=0x87).contains(&hk.vk) || hk.vk == 0x13 || hk.vk == 0x91;
        if !(hk.ctrl || hk.alt || hk.win) && !standalone_ok {
            return Err("Add Ctrl, Alt or Win (only F-keys, Pause and ScrollLock work alone)".into());
        }
        Ok(hk)
    }

    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.win {
            parts.push("Win".to_string());
        }
        parts.push(key_name(self.vk));
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_combos() {
        let hk = Hotkey::parse("ctrl + alt + p").unwrap();
        assert!(hk.ctrl && hk.alt && !hk.shift && hk.vk == b'P' as u16);
        assert_eq!(hk.display(), "Ctrl+Alt+P");
        assert_eq!(Hotkey::parse("F9").unwrap().vk, 0x78);
        assert!(Hotkey::parse("Shift+A").is_err());
        assert!(Hotkey::parse("Ctrl+Alt").is_err());
        assert!(Hotkey::parse("Ctrl+A+B").is_err());
    }
}
