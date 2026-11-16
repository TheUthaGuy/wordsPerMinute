use crate::hotkey::HotkeyAction;
use crate::stats::StatKind;
use crate::theme;
use serde::{Deserialize, Serialize};

/// Unmultiplied sRGBA.
pub type Rgba = [u8; 4];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub background: Rgba,
    pub panel: Rgba,
    pub text: Rgba,
    pub weak_text: Rgba,
    pub accent: Rgba,
    pub chart_primary: Rgba,
    pub chart_secondary: Rgba,
    pub rounding: f32,
    pub font_size: f32,
    pub spacing: f32,
}

impl Default for Theme {
    fn default() -> Self {
        theme::presets().remove(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Custom,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::TopCenter,
        Anchor::TopRight,
        Anchor::MiddleLeft,
        Anchor::MiddleRight,
        Anchor::BottomLeft,
        Anchor::BottomCenter,
        Anchor::BottomRight,
        Anchor::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Anchor::TopLeft => "Top left",
            Anchor::TopCenter => "Top center",
            Anchor::TopRight => "Top right",
            Anchor::MiddleLeft => "Middle left",
            Anchor::MiddleRight => "Middle right",
            Anchor::BottomLeft => "Bottom left",
            Anchor::BottomCenter => "Bottom center",
            Anchor::BottomRight => "Bottom right",
            Anchor::Custom => "Custom (drag it)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayLayout {
    Minimal,
    Horizontal,
    Vertical,
    Compact,
}

impl OverlayLayout {
    pub const ALL: [OverlayLayout; 4] = [
        OverlayLayout::Minimal,
        OverlayLayout::Horizontal,
        OverlayLayout::Vertical,
        OverlayLayout::Compact,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OverlayLayout::Minimal => "Minimal pill",
            OverlayLayout::Horizontal => "Horizontal tiles",
            OverlayLayout::Vertical => "Vertical list",
            OverlayLayout::Compact => "Compact line",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatToggle {
    pub kind: StatKind,
    pub enabled: bool,
}

fn stat_list(enabled: &[StatKind]) -> Vec<StatToggle> {
    let mut list: Vec<StatToggle> =
        enabled.iter().map(|&kind| StatToggle { kind, enabled: true }).collect();
    for kind in StatKind::ALL {
        if !enabled.contains(&kind) {
            list.push(StatToggle { kind, enabled: false });
        }
    }
    list
}

/// Ensures every stat appears exactly once (keeps user order, appends new kinds).
fn normalize_stats(list: &mut Vec<StatToggle>) {
    let mut seen = Vec::new();
    list.retain(|t| {
        if seen.contains(&t.kind) {
            false
        } else {
            seen.push(t.kind);
            true
        }
    });
    for kind in StatKind::ALL {
        if !seen.contains(&kind) {
            list.push(StatToggle { kind, enabled: false });
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlaySettings {
    pub enabled: bool,
    pub anchor: Anchor,
    pub margin: f32,
    pub custom_pos: [f32; 2],
    /// Locked = click-through and not draggable.
    pub locked: bool,
    pub font_size: f32,
    pub background: Rgba,
    pub text: Rgba,
    pub label: Rgba,
    pub accent: Rgba,
    pub border: Rgba,
    pub border_width: f32,
    pub rounding: f32,
    pub padding: f32,
    pub spacing: f32,
    pub layout: OverlayLayout,
    pub show_labels: bool,
    pub highlight_first: bool,
    pub monospace_values: bool,
    /// Small dot that lights up while you're typing.
    pub show_dot: bool,
    /// Dim the numbers when you're not typing.
    pub dim_when_idle: bool,
    pub auto_hide: bool,
    pub auto_hide_secs: f32,
    pub stats: Vec<StatToggle>,
    /// Bumped when the default look changes; older files get the new look.
    #[serde(default)]
    pub style_version: u32,
}

pub const OVERLAY_STYLE_VERSION: u32 = 2;

impl Default for OverlaySettings {
    fn default() -> Self {
        let mut s = Self {
            enabled: true,
            anchor: Anchor::TopRight,
            margin: 24.0,
            custom_pos: [100.0, 100.0],
            locked: true,
            font_size: 20.0,
            background: [0, 0, 0, 0],
            text: [0, 0, 0, 0],
            label: [0, 0, 0, 0],
            accent: [0, 0, 0, 0],
            border: [0, 0, 0, 0],
            border_width: 0.0,
            rounding: 0.0,
            padding: 0.0,
            spacing: 0.0,
            layout: OverlayLayout::Minimal,
            show_labels: true,
            highlight_first: false,
            monospace_values: false,
            show_dot: true,
            dim_when_idle: true,
            auto_hide: false,
            auto_hide_secs: 10.0,
            stats: stat_list(&[StatKind::LiveWpm, StatKind::AverageWpm]),
            style_version: OVERLAY_STYLE_VERSION,
        };
        s.apply_default_look();
        s
    }
}

impl OverlaySettings {
    /// Resets only the visual style, keeping position, behaviour and chosen stats.
    pub fn apply_default_look(&mut self) {
        self.font_size = 20.0;
        self.background = [12, 13, 17, 165];
        self.text = [245, 246, 250, 255];
        self.label = [160, 166, 180, 255];
        self.accent = [110, 168, 255, 255];
        self.border = [255, 255, 255, 18];
        self.border_width = 1.0;
        self.rounding = 100.0; // clamped to a full pill
        self.padding = 8.0;
        self.spacing = 16.0;
        self.layout = OverlayLayout::Minimal;
        self.show_labels = true;
        self.highlight_first = false;
        self.monospace_values = false;
        self.show_dot = true;
        self.dim_when_idle = true;
        self.style_version = OVERLAY_STYLE_VERSION;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionSettings {
    /// A session ends after this many seconds without typing.
    pub idle_timeout_secs: f32,
    /// Window for the rolling "live" WPM.
    pub live_window_secs: f32,
    /// Gaps between keys longer than this don't count as active typing time.
    pub active_gap_secs: f32,
    /// Sessions with fewer typed characters are discarded.
    pub min_chars_to_save: u32,
    /// How often the in-session WPM graph takes a sample.
    pub sample_secs: f32,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 60.0,
            live_window_secs: 10.0,
            active_gap_secs: 5.0,
            min_chars_to_save: 25,
            sample_secs: 3.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    pub toggle_pause: String,
    pub end_session: String,
    pub toggle_overlay: String,
    pub toggle_dashboard: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            toggle_pause: "Ctrl+Alt+P".into(),
            end_session: "Ctrl+Alt+E".into(),
            toggle_overlay: "Ctrl+Alt+O".into(),
            toggle_dashboard: "Ctrl+Alt+D".into(),
        }
    }
}

impl HotkeySettings {
    pub fn get_mut(&mut self, action: HotkeyAction) -> &mut String {
        match action {
            HotkeyAction::TogglePause => &mut self.toggle_pause,
            HotkeyAction::EndSession => &mut self.end_session,
            HotkeyAction::ToggleOverlay => &mut self.toggle_overlay,
            HotkeyAction::ToggleDashboard => &mut self.toggle_dashboard,
        }
    }

    pub fn get(&self, action: HotkeyAction) -> &str {
        match action {
            HotkeyAction::TogglePause => &self.toggle_pause,
            HotkeyAction::EndSession => &self.end_session,
            HotkeyAction::ToggleOverlay => &self.toggle_overlay,
            HotkeyAction::ToggleDashboard => &self.toggle_dashboard,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_name: String,
    pub custom_themes: Vec<Theme>,
    pub overlay: OverlaySettings,
    pub session: SessionSettings,
    pub hotkeys: HotkeySettings,
    pub dashboard_stats: Vec<StatToggle>,
    pub open_dashboard_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_name: Theme::default().name,
            custom_themes: Vec::new(),
            overlay: OverlaySettings::default(),
            session: SessionSettings::default(),
            hotkeys: HotkeySettings::default(),
            dashboard_stats: StatKind::ALL
                .iter()
                .map(|&kind| StatToggle { kind, enabled: true })
                .collect(),
            open_dashboard_on_start: true,
        }
    }
}

impl Settings {
    /// Repairs settings loaded from an older or hand-edited file.
    pub fn normalize(&mut self) {
        normalize_stats(&mut self.overlay.stats);
        normalize_stats(&mut self.dashboard_stats);
        let s = &mut self.session;
        s.idle_timeout_secs = s.idle_timeout_secs.clamp(5.0, 3600.0);
        s.live_window_secs = s.live_window_secs.clamp(2.0, 120.0);
        s.active_gap_secs = s.active_gap_secs.clamp(0.5, 60.0);
        s.sample_secs = s.sample_secs.clamp(1.0, 60.0);
        let o = &mut self.overlay;
        if o.style_version < OVERLAY_STYLE_VERSION {
            o.apply_default_look();
        }
        o.font_size = o.font_size.clamp(8.0, 96.0);
        o.auto_hide_secs = o.auto_hide_secs.clamp(1.0, 600.0);
    }

    pub fn all_themes(&self) -> Vec<Theme> {
        let mut themes = theme::presets();
        themes.extend(self.custom_themes.iter().cloned());
        themes
    }

    pub fn current_theme(&self) -> Theme {
        self.custom_themes
            .iter()
            .find(|t| t.name == self.theme_name)
            .cloned()
            .or_else(|| theme::presets().into_iter().find(|t| t.name == self.theme_name))
            .unwrap_or_default()
    }
}
