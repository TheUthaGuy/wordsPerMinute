use crate::dashboard::UiState;
use crate::hook::{self, HookEvent};
use crate::hotkey::{Hotkey, HotkeyAction};
use crate::settings::{Anchor, HotkeySettings, OverlayLayout, OverlaySettings, Settings, Theme};
use crate::stats::{Snapshot, StatKind};
use crate::storage;
use crate::theme::{self, c};
use crate::tracker::{SessionRecord, Tracker};
use crate::tray::{Tray, TrayAction};
use chrono::{Duration as ChronoDuration, Local};
use eframe::egui::{self, Color32, FontId, Pos2, RichText, Stroke, Vec2, ViewportCommand, ViewportId};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Aggregates over saved sessions (the live session is added on top).
#[derive(Default, Clone)]
pub struct Totals {
    pub today_words: u64,
    pub today_secs: f64,
    pub today_sessions: usize,
    pub week_words: u64,
    pub all_words: u64,
}

#[derive(Default)]
struct OverlayState {
    last_size: Vec2,
    last_pos: Option<Pos2>,
    pos_sent_at: Option<Instant>,
    passthrough: Option<bool>,
}

pub struct App {
    pub(crate) settings: Settings,
    saved_settings: Settings,
    settings_changed_at: Option<Instant>,
    pub(crate) sessions: Vec<SessionRecord>,
    pub(crate) sessions_dirty_at: Option<Instant>,
    pub(crate) tracker: Tracker,
    rx: Receiver<HookEvent>,
    pub(crate) dashboard_open: bool,
    tray: Option<Tray>,
    applied_theme: Option<Theme>,
    applied_hotkeys: Option<HotkeySettings>,
    overlay: OverlayState,
    pub(crate) ui: UiState,
    pub(crate) totals: Totals,
    pub(crate) icon: Arc<egui::IconData>,
    last_tray_update: Option<Instant>,
}

impl App {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        settings: Settings,
        sessions: Vec<SessionRecord>,
        rx: Receiver<HookEvent>,
        icon: Arc<egui::IconData>,
    ) -> Self {
        let mut app = Self {
            dashboard_open: settings.open_dashboard_on_start,
            saved_settings: settings.clone(),
            settings,
            settings_changed_at: None,
            sessions,
            sessions_dirty_at: None,
            tracker: Tracker::default(),
            rx,
            tray: Tray::new(),
            applied_theme: None,
            applied_hotkeys: None,
            overlay: OverlayState::default(),
            ui: UiState::default(),
            totals: Totals::default(),
            icon,
            last_tray_update: None,
        };
        app.recompute_totals();
        app
    }

    // ---------------------------------------------------------------- data

    pub(crate) fn recompute_totals(&mut self) {
        let today = Local::now().date_naive();
        let week_start = today - ChronoDuration::days(6);
        let mut t = Totals::default();
        for s in &self.sessions {
            let day = s.start.date_naive();
            t.all_words += s.words;
            if day == today {
                t.today_words += s.words;
                t.today_secs += s.active_secs;
                t.today_sessions += 1;
            }
            if day >= week_start {
                t.week_words += s.words;
            }
        }
        self.totals = t;
    }

    fn add_session(&mut self, rec: SessionRecord) {
        self.sessions.push(rec);
        self.save_sessions_now();
    }

    pub(crate) fn save_sessions_now(&mut self) {
        if let Err(e) = storage::save_sessions(&self.sessions) {
            self.set_status(format!("Could not save sessions: {e}"));
        }
        self.sessions_dirty_at = None;
        self.recompute_totals();
    }

    pub(crate) fn set_status(&mut self, msg: impl Into<String>) {
        self.ui.status = Some((msg.into(), Instant::now()));
    }

    pub(crate) fn snapshot(&self, now: Instant) -> Snapshot {
        let mut s = Snapshot {
            today_words: self.totals.today_words,
            today_secs: self.totals.today_secs,
            all_time_words: self.totals.all_words,
            ..Default::default()
        };
        if let Some(cur) = &self.tracker.current {
            let window = self.settings.session.live_window_secs as f64;
            s.live_wpm = cur.live_wpm(now, window);
            s.avg_wpm = cur.avg_wpm();
            s.peak_wpm = cur.peak_wpm.max(s.avg_wpm);
            s.words = cur.word_count();
            s.chars = cur.typed();
            s.keystrokes = cur.keystrokes;
            s.backspaces = cur.backspaces;
            s.duration_secs = now.saturating_duration_since(cur.start).as_secs_f64();
            s.active_secs = cur.active_secs;
            s.today_words += s.words;
            s.today_secs += s.active_secs;
            s.all_time_words += s.words;
        }
        s
    }

    // ------------------------------------------------------------- actions

    pub(crate) fn run_action(&mut self, ctx: &egui::Context, action: HotkeyAction) {
        let cfg = self.settings.session.clone();
        match action {
            HotkeyAction::TogglePause => {
                let paused = !self.tracker.paused;
                if let Some(rec) = self.tracker.set_paused(paused, &cfg) {
                    self.add_session(rec);
                }
            }
            HotkeyAction::EndSession => {
                if let Some(rec) = self.tracker.finish(&cfg) {
                    self.add_session(rec);
                    self.set_status("Session saved");
                }
            }
            HotkeyAction::ToggleOverlay => {
                self.settings.overlay.enabled = !self.settings.overlay.enabled;
            }
            HotkeyAction::ToggleDashboard => {
                self.dashboard_open = !self.dashboard_open;
            }
        }
        ctx.request_repaint();
    }

    pub(crate) fn quit(&mut self, ctx: &egui::Context) {
        self.shutdown();
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
    }

    fn shutdown(&mut self) {
        let cfg = self.settings.session.clone();
        if let Some(rec) = self.tracker.finish(&cfg) {
            self.sessions.push(rec);
            let _ = storage::save_sessions(&self.sessions);
        } else if self.sessions_dirty_at.is_some() {
            let _ = storage::save_sessions(&self.sessions);
        }
        self.sessions_dirty_at = None;
        let _ = storage::save_settings(&self.settings);
        self.saved_settings = self.settings.clone();
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        let Some(tray) = &self.tray else { return };
        for action in tray.poll() {
            match action {
                TrayAction::OpenDashboard => {
                    if self.dashboard_open {
                        ctx.send_viewport_cmd_to(dashboard_id(), ViewportCommand::Focus);
                    }
                    self.dashboard_open = true;
                }
                TrayAction::TogglePause => self.run_action(ctx, HotkeyAction::TogglePause),
                TrayAction::ToggleOverlay => self.run_action(ctx, HotkeyAction::ToggleOverlay),
                TrayAction::EndSession => self.run_action(ctx, HotkeyAction::EndSession),
                TrayAction::Quit => self.quit(ctx),
            }
        }
    }

    fn update_tray(&mut self, now: Instant) {
        if self
            .last_tray_update
            .is_some_and(|t| now.saturating_duration_since(t) < Duration::from_millis(500))
        {
            return;
        }
        self.last_tray_update = Some(now);
        let snap = self.snapshot(now);
        let tip = if self.tracker.paused {
            "WPM Counter: paused".to_string()
        } else if self.tracker.current.is_some() {
            format!(
                "WPM Counter: {:.0} WPM (avg {:.0}), {} words",
                snap.live_wpm, snap.avg_wpm, snap.words
            )
        } else {
            format!("WPM Counter: idle, {} words today", snap.today_words)
        };
        let paused = self.tracker.paused;
        let overlay_on = self.settings.overlay.enabled;
        let in_session = self.tracker.current.is_some();
        if let Some(tray) = &mut self.tray {
            tray.update(tip, paused, overlay_on, in_session);
        }
    }

    // ----------------------------------------------------- settings sync

    fn sync_theme(&mut self, ctx: &egui::Context) {
        let t = self.settings.current_theme();
        if self.applied_theme.as_ref() != Some(&t) {
            theme::apply(ctx, &t);
            self.applied_theme = Some(t);
        }
    }

    fn sync_hotkeys(&mut self) {
        if self.applied_hotkeys.as_ref() == Some(&self.settings.hotkeys) {
            return;
        }
        let list = HotkeyAction::ALL
            .iter()
            .filter_map(|&a| Hotkey::parse(self.settings.hotkeys.get(a)).ok().map(|hk| (hk, a)))
            .collect();
        hook::set_hotkeys(list);
        self.applied_hotkeys = Some(self.settings.hotkeys.clone());
    }

    /// Saves settings shortly after they stop changing (avoids writes while dragging sliders).
    fn persist(&mut self, now: Instant) {
        if self.settings != self.saved_settings {
            let since = *self.settings_changed_at.get_or_insert(now);
            if now.saturating_duration_since(since) > Duration::from_millis(600) {
                if let Err(e) = storage::save_settings(&self.settings) {
                    self.set_status(format!("Could not save settings: {e}"));
                }
                self.saved_settings = self.settings.clone();
                self.settings_changed_at = None;
            }
        } else {
            self.settings_changed_at = None;
        }
        if self
            .sessions_dirty_at
            .is_some_and(|t| now.saturating_duration_since(t) > Duration::from_secs(1))
        {
            self.save_sessions_now();
        }
    }

    // ------------------------------------------------------------- overlay

    fn draw_overlay(&mut self, ctx: &egui::Context, now: Instant) {
        let o = self.settings.overlay.clone();
        let since_activity = self
            .tracker
            .last_activity
            .map(|t| now.saturating_duration_since(t).as_secs_f32())
            .unwrap_or(f32::MAX);
        let want_visible = o.enabled
            && (!o.auto_hide || !o.locked || self.tracker.paused || since_activity <= o.auto_hide_secs);
        let opacity =
            ctx.animate_bool_with_time(egui::Id::new("overlay_fade"), want_visible, 0.35);

        let passthrough = o.locked || opacity < 0.01;
        if self.overlay.passthrough != Some(passthrough) {
            ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(passthrough));
            self.overlay.passthrough = Some(passthrough);
        }
        fix_layered_overlay();
        if opacity < 0.01 {
            return;
        }

        let snap = self.snapshot(now);
        let paused = self.tracker.paused;
        let resp = egui::Area::new(egui::Id::new("overlay_area"))
            .fixed_pos(Pos2::ZERO)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                overlay_contents(ui, &o, &snap, paused);
            });

        // Fit the window to the content.
        let size = resp.response.rect.size().ceil();
        if (size - self.overlay.last_size).length() > 0.5 {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
            self.overlay.last_size = size;
        }

        if !o.locked {
            let drag = resp.response.interact(egui::Sense::drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                self.settings.overlay.anchor = Anchor::Custom;
            }
        }
        self.position_overlay(ctx, now);
    }

    fn position_overlay(&mut self, ctx: &egui::Context, now: Instant) {
        let o = &self.settings.overlay;
        let size = self.overlay.last_size;
        let target = if o.anchor == Anchor::Custom {
            // While unlocked, adopt wherever the user dragged the window to,
            // once our own last move request has had time to apply.
            let settled = self
                .overlay
                .pos_sent_at
                .is_some_and(|t| now.saturating_duration_since(t) > Duration::from_millis(500));
            if !o.locked && settled {
                if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                    let p = rect.min;
                    if self.overlay.last_pos.is_some_and(|lp| (lp - p).length() > 0.5) {
                        self.settings.overlay.custom_pos = [p.x, p.y];
                        self.overlay.last_pos = Some(p);
                    }
                }
            }
            let p = self.settings.overlay.custom_pos;
            Pos2::new(p[0], p[1])
        } else {
            let Some(mon) = ctx.input(|i| i.viewport().monitor_size) else { return };
            let m = o.margin;
            let (w, h) = (size.x, size.y);
            let (left, cx, right) = (m, (mon.x - w) / 2.0, mon.x - w - m);
            let (top, cy, bottom) = (m, (mon.y - h) / 2.0, mon.y - h - m);
            let (x, y) = match o.anchor {
                Anchor::TopLeft => (left, top),
                Anchor::TopCenter => (cx, top),
                Anchor::TopRight => (right, top),
                Anchor::MiddleLeft => (left, cy),
                Anchor::MiddleRight => (right, cy),
                Anchor::BottomLeft => (left, bottom),
                Anchor::BottomCenter => (cx, bottom),
                Anchor::BottomRight | Anchor::Custom => (right, bottom),
            };
            Pos2::new(x.round(), y.round())
        };
        if self.overlay.last_pos.map_or(true, |lp| (lp - target).length() > 0.5) {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(target));
            self.overlay.last_pos = Some(target);
            self.overlay.pos_sent_at = Some(now);
        }
    }
}

pub const OVERLAY_TITLE: &str = "WPM Counter Overlay";

/// Click-through on Windows makes the window `WS_EX_LAYERED`. A layered window
/// is not drawn at all until its attributes are set, so set full opacity.
fn fix_layered_overlay() {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetLayeredWindowAttributes, GetWindowLongW, GetWindowThreadProcessId,
        SetLayeredWindowAttributes, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
    };
    thread_local! {
        static HWND_CACHE: std::cell::Cell<HWND> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    }
    unsafe {
        let mut hwnd = HWND_CACHE.with(|c| c.get());
        if hwnd.is_null() {
            let title: Vec<u16> = OVERLAY_TITLE.encode_utf16().chain(Some(0)).collect();
            hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if hwnd.is_null() {
                return;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != GetCurrentProcessId() {
                return;
            }
            HWND_CACHE.with(|c| c.set(hwnd));
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_LAYERED == 0 {
            return;
        }
        let (mut key, mut alpha, mut flags) = (0u32, 0u8, 0u32);
        let ok = GetLayeredWindowAttributes(hwnd, &mut key, &mut alpha, &mut flags);
        if ok == 0 || flags & LWA_ALPHA == 0 || alpha != 255 {
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        }
    }
}

pub fn dashboard_id() -> ViewportId {
    ViewportId::from_hash_of("wpm_dashboard")
}

fn text(ui: &mut egui::Ui, s: impl Into<String>, size: f32, color: Color32, mono: bool) {
    let font = if mono { FontId::monospace(size) } else { FontId::proportional(size) };
    ui.add(
        egui::Label::new(RichText::new(s).font(font).color(color))
            .extend()
            .selectable(false),
    );
}

fn overlay_contents(ui: &mut egui::Ui, o: &OverlaySettings, snap: &Snapshot, paused: bool) {
    let stats: Vec<StatKind> = o.stats.iter().filter(|t| t.enabled).map(|t| t.kind).collect();
    let fs = o.font_size;
    let label_size = (fs * 0.5).max(8.0);
    let value_style = |i: usize| -> (f32, Color32) {
        if i == 0 && o.highlight_first {
            (fs * 1.3, c(o.accent))
        } else {
            (fs, c(o.text))
        }
    };

    egui::Frame::none()
        .fill(c(o.background))
        .stroke(Stroke::new(o.border_width, c(o.border)))
        .rounding(o.rounding)
        .inner_margin(o.padding)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(o.spacing, (o.spacing * 0.3).max(2.0));
            if paused {
                text(ui, "PAUSED", label_size * 1.2, c(o.accent), false);
            }
            if stats.is_empty() {
                text(ui, "No stats selected", label_size * 1.2, c(o.label), false);
                return;
            }
            match o.layout {
                OverlayLayout::Horizontal => {
                    ui.horizontal(|ui| {
                        for (i, k) in stats.iter().enumerate() {
                            let (size, color) = value_style(i);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                text(ui, snap.value(*k), size, color, o.monospace_values);
                                if o.show_labels {
                                    text(ui, k.short(), label_size, c(o.label), false);
                                }
                            });
                        }
                    });
                }
                OverlayLayout::Vertical => {
                    egui::Grid::new("overlay_grid")
                        .num_columns(2)
                        .spacing(egui::vec2(o.spacing, (o.spacing * 0.25).max(2.0)))
                        .show(ui, |ui| {
                            for (i, k) in stats.iter().enumerate() {
                                let (size, color) = value_style(i);
                                if o.show_labels {
                                    text(ui, k.label(), label_size * 1.2, c(o.label), false);
                                }
                                text(ui, snap.value(*k), size, color, o.monospace_values);
                                ui.end_row();
                            }
                        });
                }
                OverlayLayout::Compact => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = (o.spacing * 0.35).max(3.0);
                        for (i, k) in stats.iter().enumerate() {
                            if i > 0 {
                                text(ui, "·", fs, c(o.label), false);
                            }
                            let (_, color) = value_style(i);
                            text(ui, snap.value(*k), fs, color, o.monospace_values);
                            if o.show_labels {
                                text(ui, k.short().to_lowercase(), fs * 0.7, c(o.label), false);
                            }
                        }
                    });
                }
            }
        });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();

        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                HookEvent::Key { kind, at } => {
                    let cfg = &self.settings.session;
                    if let Some(rec) = self.tracker.on_key(kind, at, cfg) {
                        self.add_session(rec);
                    }
                }
                HookEvent::Hotkey(action) => self.run_action(ctx, action),
            }
        }
        self.poll_tray(ctx);
        if let Some(rec) = self.tracker.tick(now, &self.settings.session) {
            self.add_session(rec);
        }

        self.sync_theme(ctx);
        self.sync_hotkeys();
        self.draw_overlay(ctx, now);
        if self.dashboard_open {
            self.show_dashboard(ctx);
        }
        self.update_tray(now);
        self.persist(now);

        // Keep the live numbers moving and the idle timeout ticking.
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent overlay window
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown();
    }
}
