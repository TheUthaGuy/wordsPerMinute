use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    OpenDashboard,
    TogglePause,
    ToggleOverlay,
    EndSession,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    open: MenuItem,
    pause: MenuItem,
    overlay: MenuItem,
    end: MenuItem,
    quit: MenuItem,
    last_tooltip: String,
}

impl Tray {
    pub fn new() -> Option<Self> {
        let open = MenuItem::new("Open dashboard", true, None);
        let pause = MenuItem::new("Pause tracking", true, None);
        let overlay = MenuItem::new("Hide overlay", true, None);
        let end = MenuItem::new("End current session", true, None);
        let quit = MenuItem::new("Quit", true, None);

        let menu = Menu::new();
        menu.append_items(&[
            &open,
            &PredefinedMenuItem::separator(),
            &pause,
            &overlay,
            &end,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .ok()?;

        let size = 32;
        let icon = tray_icon::Icon::from_rgba(crate::icon::rgba(size), size, size).ok()?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("WPM Counter")
            .with_icon(icon)
            .build()
            .ok()?;

        Some(Self { icon, open, pause, overlay, end, quit, last_tooltip: String::new() })
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();
        while let Ok(e) = MenuEvent::receiver().try_recv() {
            let id = e.id();
            if id == self.open.id() {
                actions.push(TrayAction::OpenDashboard);
            } else if id == self.pause.id() {
                actions.push(TrayAction::TogglePause);
            } else if id == self.overlay.id() {
                actions.push(TrayAction::ToggleOverlay);
            } else if id == self.end.id() {
                actions.push(TrayAction::EndSession);
            } else if id == self.quit.id() {
                actions.push(TrayAction::Quit);
            }
        }
        while let Ok(e) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                actions.push(TrayAction::OpenDashboard);
            }
        }
        actions
    }

    pub fn update(&mut self, tooltip: String, paused: bool, overlay_on: bool, in_session: bool) {
        if tooltip != self.last_tooltip {
            let _ = self.icon.set_tooltip(Some(&tooltip));
            self.last_tooltip = tooltip;
        }
        self.pause.set_text(if paused { "Resume tracking" } else { "Pause tracking" });
        self.overlay.set_text(if overlay_on { "Hide overlay" } else { "Show overlay" });
        self.end.set_enabled(in_session);
    }
}
