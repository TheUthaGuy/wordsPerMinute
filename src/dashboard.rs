use crate::app::{dashboard_id, App};
use crate::hotkey::{Hotkey, HotkeyAction};
use crate::settings::{Anchor, OverlayLayout, OverlaySettings, Rgba, SessionSettings, Settings, StatToggle, Theme};
use crate::stats::{fmt_duration, fmt_num, Snapshot, StatKind};
use crate::storage;
use crate::theme::{c, mix};
use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints, Points};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Current,
    History,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Overlay,
    Dashboard,
    Sessions,
    Hotkeys,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortCol {
    #[default]
    Date,
    Name,
    Duration,
    Active,
    Avg,
    Peak,
    Words,
    Corrections,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChartRange {
    Week,
    #[default]
    Month,
    Quarter,
    All,
}

impl ChartRange {
    fn label(self) -> &'static str {
        match self {
            ChartRange::Week => "7 days",
            ChartRange::Month => "30 days",
            ChartRange::Quarter => "90 days",
            ChartRange::All => "All time",
        }
    }
    fn days(self) -> Option<i64> {
        match self {
            ChartRange::Week => Some(7),
            ChartRange::Month => Some(30),
            ChartRange::Quarter => Some(90),
            ChartRange::All => None,
        }
    }
}

pub struct UiState {
    pub tab: Tab,
    pub settings_tab: SettingsTab,
    pub sort: SortCol,
    pub sort_desc: bool,
    pub selected: Option<i64>,
    pub confirm_delete: Option<i64>,
    pub confirm_clear_all: bool,
    pub confirm_reset: bool,
    pub status: Option<(String, Instant)>,
    pub new_theme_name: String,
    pub chart_range: ChartRange,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            settings_tab: SettingsTab::default(),
            sort: SortCol::Date,
            sort_desc: true,
            selected: None,
            confirm_delete: None,
            confirm_clear_all: false,
            confirm_reset: false,
            status: None,
            new_theme_name: String::new(),
            chart_range: ChartRange::default(),
        }
    }
}

// ------------------------------------------------------------------ helpers

fn section(ui: &mut egui::Ui, t: &Theme, title: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(title).size(t.font_size * 1.15).strong().color(c(t.text)));
    ui.add_space(4.0);
}

fn weak(t: &Theme, s: impl Into<String>) -> RichText {
    RichText::new(s).color(c(t.weak_text))
}

fn card_frame(t: &Theme) -> egui::Frame {
    egui::Frame::none()
        .fill(c(t.panel))
        .rounding(t.rounding)
        .stroke(Stroke::new(1.0_f32, mix(c(t.panel), c(t.text), 0.10)))
        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
}

fn stat_card(ui: &mut egui::Ui, t: &Theme, label: &str, value: &str, hint: &str) {
    card_frame(t)
        .show(ui, |ui| {
            ui.set_width(150.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(label).size(t.font_size * 0.85).color(c(t.weak_text)));
                ui.label(
                    RichText::new(value).size(t.font_size * 1.9).strong().color(c(t.accent)),
                );
            });
        })
        .response
        .on_hover_text(hint);
}

fn color_edit(ui: &mut egui::Ui, rgba: &mut Rgba, alpha: bool) {
    let mut col = c(*rgba);
    let mode = if alpha {
        egui::color_picker::Alpha::OnlyBlend
    } else {
        egui::color_picker::Alpha::Opaque
    };
    if egui::color_picker::color_edit_button_srgba(ui, &mut col, mode).changed() {
        *rgba = col.to_srgba_unmultiplied();
    }
}

fn color_row(ui: &mut egui::Ui, label: &str, rgba: &mut Rgba, alpha: bool) {
    ui.label(label);
    color_edit(ui, rgba, alpha);
    ui.end_row();
}

fn stat_list_editor(ui: &mut egui::Ui, t: &Theme, id: &str, list: &mut [StatToggle]) {
    let mut swap = None;
    egui::Grid::new(id).striped(true).num_columns(4).show(ui, |ui| {
        let n = list.len();
        for i in 0..n {
            let kind = list[i].kind;
            ui.checkbox(&mut list[i].enabled, kind.label());
            if ui.add_enabled(i > 0, egui::Button::new("Up").small()).clicked() {
                swap = Some((i, i - 1));
            }
            if ui.add_enabled(i + 1 < n, egui::Button::new("Down").small()).clicked() {
                swap = Some((i, i + 1));
            }
            ui.label(weak(t, kind.description()));
            ui.end_row();
        }
    });
    if let Some((a, b)) = swap {
        list.swap(a, b);
    }
}

fn wpm_plot_line(points: Vec<[f64; 2]>, color: Color32) -> Line {
    Line::new(PlotPoints::from(points)).color(color).width(2.0_f32).fill(0.0_f32).name("WPM")
}

fn session_plot(ui: &mut egui::Ui, id: &str, t: &Theme, points: Vec<[f64; 2]>, live: Option<[f64; 2]>) {
    let color = c(t.chart_primary);
    Plot::new(id)
        .height(220.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_x(0.0)
        .include_y(0.0)
        .include_y(60.0)
        .y_axis_label("WPM")
        .x_axis_formatter(|mark, _| fmt_duration(mark.value))
        .label_formatter(|_, v| format!("{:.0} WPM at {}", v.y, fmt_duration(v.x)))
        .show(ui, |plot| {
            plot.line(wpm_plot_line(points, color));
            if let Some(p) = live {
                plot.points(Points::new(vec![p]).radius(4.0_f32).color(c(t.chart_secondary)).name("Now"));
            }
        });
}

fn hotkey_hint(settings: &Settings, action: HotkeyAction) -> String {
    match Hotkey::parse(settings.hotkeys.get(action)) {
        Ok(hk) => format!(" ({})", hk.display()),
        Err(_) => String::new(),
    }
}

// ---------------------------------------------------------------- dashboard

impl App {
    pub(crate) fn show_dashboard(&mut self, ctx: &egui::Context) {
        let builder = egui::ViewportBuilder::default()
            .with_title("WPM Counter")
            .with_inner_size([1080.0, 760.0])
            .with_min_inner_size([760.0, 500.0])
            .with_icon(self.icon.clone());
        ctx.show_viewport_immediate(dashboard_id(), builder, |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                // Closing the dashboard only hides it; tracking continues in the tray.
                self.dashboard_open = false;
                return;
            }
            self.dashboard_ui(ctx);
        });
    }

    fn dashboard_ui(&mut self, ctx: &egui::Context) {
        let t = self.settings.current_theme();
        let now = Instant::now();

        egui::TopBottomPanel::top("dash_top")
            .frame(
                egui::Frame::none()
                    .fill(c(t.panel))
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("WPM Counter").size(t.font_size * 1.4).strong().color(c(t.accent)));
                    ui.add_space(20.0);
                    for (tab, name) in [
                        (Tab::Current, "Current session"),
                        (Tab::History, "Past sessions"),
                        (Tab::Settings, "Settings"),
                    ] {
                        if ui.selectable_label(self.ui.tab == tab, RichText::new(name).size(t.font_size * 1.05)).clicked() {
                            self.ui.tab = tab;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let paused = self.tracker.paused;
                        let hint = hotkey_hint(&self.settings, HotkeyAction::TogglePause);
                        let label = if paused { "Resume" } else { "Pause" };
                        if ui.button(label).on_hover_text(format!("Pause/resume tracking{hint}")).clicked() {
                            self.run_action(ctx, HotkeyAction::TogglePause);
                        }
                        let (dot, status) = if paused {
                            (c(t.chart_secondary), "Paused")
                        } else if self.tracker.current.is_some() {
                            (Color32::from_rgb(74, 222, 128), "Session active")
                        } else {
                            (c(t.weak_text), "Waiting for typing")
                        };
                        ui.label(weak(&t, status));
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 5.0, dot);
                    });
                });
            });

        if let Some((msg, at)) = self.ui.status.clone() {
            if now.saturating_duration_since(at) < Duration::from_secs(5) {
                egui::TopBottomPanel::bottom("dash_status")
                    .frame(egui::Frame::none().fill(c(t.panel)).inner_margin(egui::Margin::symmetric(16.0, 6.0)))
                    .show(ctx, |ui| {
                        ui.label(weak(&t, msg));
                    });
            } else {
                self.ui.status = None;
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(c(t.background)).inner_margin(egui::Margin::symmetric(20.0, 12.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.ui.tab {
                    Tab::Current => self.current_tab(ui, ctx, &t, now),
                    Tab::History => self.history_tab(ui, &t),
                    Tab::Settings => self.settings_tab(ui, ctx, &t),
                });
            });
    }

    // ------------------------------------------------------------ current

    fn current_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: &Theme, now: Instant) {
        let snap: Snapshot = self.snapshot(now);
        ui.horizontal(|ui| {
            let headline = match &self.tracker.current {
                Some(cur) => format!("Session started at {}", cur.start_wall.format("%H:%M:%S")),
                None if self.tracker.paused => "Tracking is paused".to_string(),
                None => "Start typing anywhere and a session will begin automatically".to_string(),
            };
            ui.label(RichText::new(headline).size(t.font_size * 1.1).color(c(t.text)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let hint = hotkey_hint(&self.settings, HotkeyAction::EndSession);
                if ui
                    .add_enabled(self.tracker.current.is_some(), egui::Button::new("End session"))
                    .on_hover_text(format!("Save this session now and start fresh{hint}"))
                    .clicked()
                {
                    self.run_action(ctx, HotkeyAction::EndSession);
                }
            });
        });
        ui.label(weak(
            t,
            format!(
                "Sessions end after {} of no typing. Typing inside this window isn't counted.",
                fmt_duration(self.settings.session.idle_timeout_secs as f64)
            ),
        ));

        section(ui, t, "Stats");
        let kinds: Vec<StatKind> =
            self.settings.dashboard_stats.iter().filter(|s| s.enabled).map(|s| s.kind).collect();
        if kinds.is_empty() {
            ui.label(weak(t, "No stat cards selected. Choose some in Settings > Dashboard."));
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            for k in kinds {
                stat_card(ui, t, k.label(), &snap.value(k), k.description());
            }
        });

        section(ui, t, "WPM this session");
        match &self.tracker.current {
            Some(cur) => {
                let pts: Vec<[f64; 2]> =
                    cur.timeline.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
                let live = [snap.duration_secs, snap.live_wpm];
                session_plot(ui, "live_plot", t, pts, Some(live));
            }
            None => {
                ui.label(weak(t, "The graph will appear once you start typing."));
            }
        }

        section(ui, t, "Today");
        ui.label(format!(
            "{} words in {} sessions, {} of active typing.  This week: {} words.",
            fmt_num(snap.today_words),
            self.totals.today_sessions + self.tracker.current.is_some() as usize,
            fmt_duration(snap.today_secs),
            fmt_num(self.totals.week_words + snap.words),
        ));
    }

    // ------------------------------------------------------------ history

    fn history_tab(&mut self, ui: &mut egui::Ui, t: &Theme) {
        if self.sessions.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No saved sessions yet").size(t.font_size * 1.3));
                ui.label(weak(
                    t,
                    "Sessions are saved when you stop typing for a while, or when you end one manually.",
                ));
            });
            return;
        }

        // Summary.
        let total_typed: u64 = self.sessions.iter().map(|s| s.typed()).sum();
        let total_active: f64 = self.sessions.iter().map(|s| s.active_secs).sum();
        let overall = if total_active > 1.0 { total_typed as f64 / 5.0 / (total_active / 60.0) } else { 0.0 };
        let best_avg = self.sessions.iter().map(|s| s.avg_wpm()).fold(0.0, f64::max);
        let best_peak = self.sessions.iter().map(|s| s.peak_wpm).fold(0.0, f64::max);

        section(ui, t, "Summary");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            stat_card(ui, t, "Sessions", &fmt_num(self.sessions.len() as u64), "Saved sessions");
            stat_card(ui, t, "Overall avg WPM", &format!("{overall:.0}"), "Weighted by active typing time");
            stat_card(ui, t, "Best session avg", &format!("{best_avg:.0}"), "Highest session average WPM");
            stat_card(ui, t, "Best peak WPM", &format!("{best_peak:.0}"), "Fastest rolling WPM ever reached");
            stat_card(ui, t, "Total words", &fmt_num(self.totals.all_words), "Across all saved sessions");
            stat_card(ui, t, "Total typing time", &fmt_duration(total_active), "Active typing time across all sessions");
            stat_card(ui, t, "Words today", &fmt_num(self.totals.today_words), "Saved sessions from today");
            stat_card(ui, t, "Words this week", &fmt_num(self.totals.week_words), "Saved sessions from the last 7 days");
        });

        // Daily chart.
        section(ui, t, "Average WPM per day");
        ui.horizontal(|ui| {
            for r in [ChartRange::Week, ChartRange::Month, ChartRange::Quarter, ChartRange::All] {
                ui.selectable_value(&mut self.ui.chart_range, r, r.label());
            }
        });
        self.daily_chart(ui, t);

        // Table.
        section(ui, t, "Sessions");
        ui.horizontal(|ui| {
            ui.label(weak(t, "Click a row to see details. Click a column header to sort."));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Export JSON").clicked() {
                    self.export(false);
                }
                if ui.button("Export CSV").clicked() {
                    self.export(true);
                }
            });
        });
        self.sessions_table(ui, t);

        // Details.
        if let Some(id) = self.ui.selected {
            if let Some(idx) = self.sessions.iter().position(|s| s.id == id) {
                self.session_details(ui, t, idx);
            } else {
                self.ui.selected = None;
            }
        }
    }

    fn daily_chart(&mut self, ui: &mut egui::Ui, t: &Theme) {
        let today = Local::now().date_naive();
        let from = self.ui.chart_range.days().map(|d| today - ChronoDuration::days(d - 1));
        let mut days: BTreeMap<NaiveDate, (u64, f64, u64)> = BTreeMap::new();
        for s in &self.sessions {
            let d = s.start.date_naive();
            if from.is_some_and(|f| d < f) {
                continue;
            }
            let e = days.entry(d).or_default();
            e.0 += s.typed();
            e.1 += s.active_secs;
            e.2 += s.words;
        }
        if days.is_empty() {
            ui.label(weak(t, "No sessions in this range."));
            return;
        }
        let bars: Vec<Bar> = days
            .iter()
            .map(|(d, (typed, secs, words))| {
                let wpm = if *secs > 1.0 { *typed as f64 / 5.0 / (secs / 60.0) } else { 0.0 };
                Bar::new(d.num_days_from_ce() as f64, wpm)
                    .width(0.7)
                    .fill(c(t.chart_primary))
                    .name(format!("{}: {:.0} WPM, {} words", d.format("%a %b %d"), wpm, fmt_num(*words)))
            })
            .collect();
        let mut plot = Plot::new("daily_plot")
            .height(220.0)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .include_y(0.0)
            .y_axis_label("WPM")
            .x_axis_formatter(|mark, _| {
                if (mark.value - mark.value.round()).abs() > 0.01 {
                    return String::new();
                }
                NaiveDate::from_num_days_from_ce_opt(mark.value.round() as i32)
                    .map(|d| d.format("%b %d").to_string())
                    .unwrap_or_default()
            })
            .label_formatter(|name, _| name.to_string());
        if let Some(f) = from {
            plot = plot
                .include_x(f.num_days_from_ce() as f64 - 0.5)
                .include_x(today.num_days_from_ce() as f64 + 0.5);
        }
        plot.show(ui, |p| p.bar_chart(BarChart::new(bars).color(c(t.chart_primary))));
    }

    fn sorted_indices(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.sessions.len()).collect();
        let s = &self.sessions;
        let cmp_f = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal);
        idx.sort_by(|&a, &b| {
            let (x, y) = (&s[a], &s[b]);
            match self.ui.sort {
                SortCol::Date => x.start.cmp(&y.start),
                SortCol::Name => x.display_name().to_lowercase().cmp(&y.display_name().to_lowercase()),
                SortCol::Duration => cmp_f(x.duration_secs(), y.duration_secs()),
                SortCol::Active => cmp_f(x.active_secs, y.active_secs),
                SortCol::Avg => cmp_f(x.avg_wpm(), y.avg_wpm()),
                SortCol::Peak => cmp_f(x.peak_wpm, y.peak_wpm),
                SortCol::Words => x.words.cmp(&y.words),
                SortCol::Corrections => cmp_f(x.correction_rate(), y.correction_rate()),
            }
        });
        if self.ui.sort_desc {
            idx.reverse();
        }
        idx
    }

    fn sessions_table(&mut self, ui: &mut egui::Ui, t: &Theme) {
        let idx = self.sorted_indices();
        let selected = self.ui.selected;
        let mut clicked = None;
        let mut sort_click = None;
        let cols = [
            (SortCol::Date, "Date"),
            (SortCol::Name, "Name"),
            (SortCol::Duration, "Length"),
            (SortCol::Active, "Active"),
            (SortCol::Avg, "Avg WPM"),
            (SortCol::Peak, "Peak WPM"),
            (SortCol::Words, "Words"),
            (SortCol::Corrections, "Corrections"),
        ];
        let accent = c(t.accent);
        let (sort, desc) = (self.ui.sort, self.ui.sort_desc);

        ui.push_id("sessions_table", |ui| {
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .sense(Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::initial(150.0).at_least(110.0))
                .column(Column::initial(200.0).at_least(100.0).clip(true))
                .columns(Column::initial(80.0).at_least(60.0), 5)
                .column(Column::remainder().at_least(80.0))
                .max_scroll_height(320.0)
                .header(26.0, |mut header| {
                    for (col, name) in cols {
                        header.col(|ui| {
                            let arrow = if sort == col { if desc { " v" } else { " ^" } } else { "" };
                            let text = RichText::new(format!("{name}{arrow}")).strong();
                            let text = if sort == col { text.color(accent) } else { text };
                            if ui.add(egui::Button::new(text).frame(false)).clicked() {
                                sort_click = Some(col);
                            }
                        });
                    }
                })
                .body(|body| {
                    body.rows(24.0, idx.len(), |mut row| {
                        let s = &self.sessions[idx[row.index()]];
                        row.set_selected(selected == Some(s.id));
                        row.col(|ui| {
                            ui.label(s.start.format("%Y-%m-%d %H:%M").to_string());
                        });
                        row.col(|ui| {
                            ui.label(s.display_name());
                        });
                        row.col(|ui| {
                            ui.label(fmt_duration(s.duration_secs()));
                        });
                        row.col(|ui| {
                            ui.label(fmt_duration(s.active_secs));
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(format!("{:.0}", s.avg_wpm())).strong());
                        });
                        row.col(|ui| {
                            ui.label(format!("{:.0}", s.peak_wpm));
                        });
                        row.col(|ui| {
                            ui.label(fmt_num(s.words));
                        });
                        row.col(|ui| {
                            ui.label(format!("{:.1}%", s.correction_rate()));
                        });
                        if row.response().clicked() {
                            clicked = Some(s.id);
                        }
                    });
                });
        });

        if let Some(col) = sort_click {
            if self.ui.sort == col {
                self.ui.sort_desc = !self.ui.sort_desc;
            } else {
                self.ui.sort = col;
                self.ui.sort_desc = matches!(col, SortCol::Date | SortCol::Avg | SortCol::Peak | SortCol::Words);
            }
        }
        if let Some(id) = clicked {
            self.ui.selected = if selected == Some(id) { None } else { Some(id) };
            self.ui.confirm_delete = None;
        }
    }

    fn session_details(&mut self, ui: &mut egui::Ui, t: &Theme, idx: usize) {
        section(ui, t, "Session details");
        let mut close = false;
        let mut delete = false;
        card_frame(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let s = &mut self.sessions[idx];
            let placeholder = s.display_name();
            ui.horizontal(|ui| {
                ui.label("Name");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut s.name)
                        .hint_text(placeholder)
                        .desired_width(280.0),
                );
                if resp.changed() {
                    self.sessions_dirty_at = Some(Instant::now());
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                    if self.ui.confirm_delete == Some(s.id) {
                        if ui.button(RichText::new("Confirm delete").color(Color32::from_rgb(239, 68, 68))).clicked() {
                            delete = true;
                        }
                        if ui.button("Cancel").clicked() {
                            self.ui.confirm_delete = None;
                        }
                    } else if ui.button("Delete").clicked() {
                        self.ui.confirm_delete = Some(s.id);
                    }
                });
            });
            ui.add_space(6.0);
            let s = &self.sessions[idx];
            ui.label(weak(
                t,
                format!(
                    "{} to {}",
                    s.start.format("%A, %B %d %Y, %H:%M:%S"),
                    s.end.format("%H:%M:%S")
                ),
            ));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                let items = [
                    ("Average WPM", format!("{:.0}", s.avg_wpm())),
                    ("Peak WPM", format!("{:.0}", s.peak_wpm)),
                    ("Words", fmt_num(s.words)),
                    ("Characters", fmt_num(s.typed())),
                    ("Keystrokes", fmt_num(s.keystrokes)),
                    ("Backspaces", fmt_num(s.backspaces)),
                    ("Correction rate", format!("{:.1}%", s.correction_rate())),
                    ("Session length", fmt_duration(s.duration_secs())),
                    ("Active time", fmt_duration(s.active_secs)),
                ];
                for (label, value) in items {
                    stat_card(ui, t, label, &value, label);
                }
            });
            ui.add_space(8.0);
            if s.timeline.is_empty() {
                ui.label(weak(t, "No graph data for this session."));
            } else {
                let pts = s.timeline.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
                session_plot(ui, "detail_plot", t, pts, None);
            }
        });
        if delete {
            self.sessions.remove(idx);
            self.ui.selected = None;
            self.ui.confirm_delete = None;
            self.save_sessions_now();
            self.set_status("Session deleted");
        } else if close {
            self.ui.selected = None;
            self.ui.confirm_delete = None;
        }
    }

    fn export(&mut self, csv: bool) {
        let (ext, name) = if csv { ("csv", "wpm-sessions.csv") } else { ("json", "wpm-sessions.json") };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(ext.to_uppercase(), &[ext])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        let res = if csv {
            storage::export_csv(&path, &self.sessions)
        } else {
            storage::export_json(&path, &self.sessions)
        };
        match res {
            Ok(()) => self.set_status(format!("Exported {} sessions to {}", self.sessions.len(), path.display())),
            Err(e) => self.set_status(format!("Export failed: {e}")),
        }
    }

    // ----------------------------------------------------------- settings

    fn settings_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: &Theme) {
        ui.horizontal(|ui| {
            for (tab, name) in [
                (SettingsTab::Appearance, "Appearance"),
                (SettingsTab::Overlay, "Overlay"),
                (SettingsTab::Dashboard, "Dashboard"),
                (SettingsTab::Sessions, "Sessions"),
                (SettingsTab::Hotkeys, "Hotkeys"),
                (SettingsTab::Data, "Data"),
            ] {
                ui.selectable_value(&mut self.ui.settings_tab, tab, name);
            }
        });
        ui.separator();
        match self.ui.settings_tab {
            SettingsTab::Appearance => self.appearance_settings(ui, t),
            SettingsTab::Overlay => overlay_settings(ui, t, &mut self.settings.overlay),
            SettingsTab::Dashboard => {
                section(ui, t, "Stat cards on the Current session tab");
                stat_list_editor(ui, t, "dash_stats", &mut self.settings.dashboard_stats);
                section(ui, t, "Startup");
                ui.checkbox(
                    &mut self.settings.open_dashboard_on_start,
                    "Open the dashboard when the app starts",
                );
                ui.label(weak(t, "Closing the dashboard keeps the app running in the system tray."));
            }
            SettingsTab::Sessions => session_settings(ui, t, &mut self.settings.session),
            SettingsTab::Hotkeys => hotkey_settings(ui, t, &mut self.settings),
            SettingsTab::Data => self.data_settings(ui, ctx, t),
        }
    }

    fn appearance_settings(&mut self, ui: &mut egui::Ui, t: &Theme) {
        section(ui, t, "Theme");
        let themes = self.settings.all_themes();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            for th in &themes {
                let selected = th.name == self.settings.theme_name;
                let resp = theme_swatch(ui, th, selected);
                if resp.clicked() {
                    self.settings.theme_name = th.name.clone();
                }
            }
        });

        section(ui, t, "Customize");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.ui.new_theme_name)
                    .hint_text("New theme name")
                    .desired_width(200.0),
            );
            if ui.button("Create from current theme").clicked() {
                let mut base = self.ui.new_theme_name.trim().to_string();
                if base.is_empty() {
                    base = format!("{} (custom)", t.name);
                }
                let mut name = base.clone();
                let mut n = 2;
                while themes.iter().any(|x| x.name == name) {
                    name = format!("{base} {n}");
                    n += 1;
                }
                let mut copy = t.clone();
                copy.name = name.clone();
                self.settings.custom_themes.push(copy);
                self.settings.theme_name = name;
                self.ui.new_theme_name.clear();
            }
        });

        let Some(idx) = self
            .settings
            .custom_themes
            .iter()
            .position(|x| x.name == self.settings.theme_name)
        else {
            ui.label(weak(t, "Built-in themes can't be edited. Create a copy above to customize every color."));
            return;
        };

        ui.add_space(8.0);
        let mut delete = false;
        let names: Vec<String> = themes.iter().map(|x| x.name.clone()).collect();
        {
            let th = &mut self.settings.custom_themes[idx];
            egui::Grid::new("theme_editor").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                let mut name = th.name.clone();
                if ui.text_edit_singleline(&mut name).changed() {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() && !names.iter().any(|n| n == trimmed && *n != th.name) {
                        th.name = trimmed.to_string();
                    }
                }
                ui.end_row();
                ui.label("Dark mode widgets");
                ui.checkbox(&mut th.dark, "");
                ui.end_row();
                color_row(ui, "Background", &mut th.background, false);
                color_row(ui, "Panels & cards", &mut th.panel, false);
                color_row(ui, "Text", &mut th.text, false);
                color_row(ui, "Muted text", &mut th.weak_text, false);
                color_row(ui, "Accent", &mut th.accent, false);
                color_row(ui, "Chart primary", &mut th.chart_primary, false);
                color_row(ui, "Chart secondary", &mut th.chart_secondary, false);
                ui.label("Corner rounding");
                ui.add(egui::Slider::new(&mut th.rounding, 0.0..=20.0));
                ui.end_row();
                ui.label("Font size");
                ui.add(egui::Slider::new(&mut th.font_size, 10.0..=24.0));
                ui.end_row();
                ui.label("Spacing");
                ui.add(egui::Slider::new(&mut th.spacing, 0.6..=1.8));
                ui.end_row();
            });
            self.settings.theme_name = th.name.clone();
            ui.add_space(8.0);
            if ui.button("Delete this theme").clicked() {
                delete = true;
            }
        }
        if delete {
            self.settings.custom_themes.remove(idx);
            self.settings.theme_name = Theme::default().name;
        }
    }

    fn data_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, t: &Theme) {
        let dir = storage::data_dir();
        section(ui, t, "Storage");
        ui.label(format!("Data folder: {}", dir.display()));
        ui.label(weak(
            t,
            "Only counts and timings are stored. The text you type is never recorded.",
        ));
        if ui.button("Open data folder").clicked() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
        }

        section(ui, t, "Export");
        ui.horizontal(|ui| {
            if ui.button("Export CSV").clicked() {
                self.export(true);
            }
            if ui.button("Export JSON").clicked() {
                self.export(false);
            }
        });

        section(ui, t, "Danger zone");
        ui.horizontal(|ui| {
            if self.ui.confirm_clear_all {
                ui.label(format!("Delete all {} sessions?", self.sessions.len()));
                if ui.button(RichText::new("Yes, delete all").color(Color32::from_rgb(239, 68, 68))).clicked() {
                    self.sessions.clear();
                    self.ui.selected = None;
                    self.ui.confirm_clear_all = false;
                    self.save_sessions_now();
                    self.set_status("All sessions deleted");
                }
                if ui.button("Cancel").clicked() {
                    self.ui.confirm_clear_all = false;
                }
            } else if ui.button("Delete all sessions").clicked() {
                self.ui.confirm_clear_all = true;
            }
        });
        ui.horizontal(|ui| {
            if self.ui.confirm_reset {
                ui.label("Reset every setting, including custom themes?");
                if ui.button(RichText::new("Yes, reset").color(Color32::from_rgb(239, 68, 68))).clicked() {
                    self.settings = Settings::default();
                    self.ui.confirm_reset = false;
                    self.set_status("Settings reset to defaults");
                }
                if ui.button("Cancel").clicked() {
                    self.ui.confirm_reset = false;
                }
            } else if ui.button("Reset all settings").clicked() {
                self.ui.confirm_reset = true;
            }
        });

        section(ui, t, "App");
        if ui.button("Quit WPM Counter").clicked() {
            self.quit(ctx);
        }
    }
}

fn theme_swatch(ui: &mut egui::Ui, th: &Theme, selected: bool) -> egui::Response {
    let size = egui::vec2(130.0, 78.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let r = 8.0;
    p.rect_filled(rect, r, c(th.background));
    let inner = rect.shrink(8.0);
    let card = egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), 30.0));
    p.rect_filled(card, 4.0, c(th.panel));
    p.rect_filled(
        egui::Rect::from_min_size(card.min + egui::vec2(6.0, 8.0), egui::vec2(40.0, 6.0)),
        2.0,
        c(th.accent),
    );
    p.rect_filled(
        egui::Rect::from_min_size(card.min + egui::vec2(6.0, 18.0), egui::vec2(60.0, 4.0)),
        2.0,
        c(th.weak_text),
    );
    p.text(
        egui::pos2(inner.min.x, inner.max.y),
        egui::Align2::LEFT_BOTTOM,
        &th.name,
        egui::FontId::proportional(12.0),
        c(th.text),
    );
    let stroke = if selected {
        Stroke::new(2.5_f32, c(th.accent))
    } else if resp.hovered() {
        Stroke::new(1.5_f32, mix(c(th.background), c(th.text), 0.5))
    } else {
        Stroke::new(1.0_f32, mix(c(th.background), c(th.text), 0.2))
    };
    p.rect_stroke(rect, r, stroke);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn overlay_settings(ui: &mut egui::Ui, t: &Theme, o: &mut OverlaySettings) {
    section(ui, t, "Visibility");
    ui.checkbox(&mut o.enabled, "Show the overlay");
    ui.horizontal(|ui| {
        ui.checkbox(&mut o.auto_hide, "Auto-hide when not typing, after");
        ui.add_enabled(
            o.auto_hide,
            egui::DragValue::new(&mut o.auto_hide_secs).range(1.0..=600.0).suffix(" s"),
        );
    });
    ui.checkbox(&mut o.locked, "Lock position (clicks pass through the overlay)");
    if !o.locked {
        ui.label(weak(t, "Unlocked: drag the overlay with the mouse to place it anywhere. Lock it again when done."));
    }

    section(ui, t, "Position");
    egui::Grid::new("ov_pos").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label("Anchor");
        egui::ComboBox::from_id_salt("ov_anchor")
            .selected_text(o.anchor.label())
            .show_ui(ui, |ui| {
                for a in Anchor::ALL {
                    ui.selectable_value(&mut o.anchor, a, a.label());
                }
            });
        ui.end_row();
        ui.label("Distance from screen edge");
        ui.add_enabled(o.anchor != Anchor::Custom, egui::Slider::new(&mut o.margin, 0.0..=300.0).suffix(" px"));
        ui.end_row();
    });

    section(ui, t, "Content");
    egui::Grid::new("ov_layout").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label("Layout");
        ui.horizontal(|ui| {
            for l in OverlayLayout::ALL {
                ui.selectable_value(&mut o.layout, l, l.label());
            }
        });
        ui.end_row();
        ui.label("Text size");
        ui.add(egui::Slider::new(&mut o.font_size, 10.0..=72.0));
        ui.end_row();
        ui.label("Labels");
        ui.checkbox(&mut o.show_labels, "Show stat names");
        ui.end_row();
        ui.label("Highlight");
        ui.checkbox(&mut o.highlight_first, "Make the first stat bigger and accent-colored");
        ui.end_row();
        ui.label("Numbers");
        ui.checkbox(&mut o.monospace_values, "Monospace digits (less jitter)");
        ui.end_row();
    });
    ui.add_space(6.0);
    ui.label(weak(t, "Choose which stats to show and in what order:"));
    stat_list_editor(ui, t, "ov_stats", &mut o.stats);

    section(ui, t, "Look");
    egui::Grid::new("ov_look").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        color_row(ui, "Background (with opacity)", &mut o.background, true);
        color_row(ui, "Values", &mut o.text, false);
        color_row(ui, "Labels", &mut o.label, false);
        color_row(ui, "Highlight", &mut o.accent, false);
        color_row(ui, "Border", &mut o.border, true);
        ui.label("Background opacity");
        let mut alpha = o.background[3] as f32 / 255.0 * 100.0;
        if ui.add(egui::Slider::new(&mut alpha, 0.0..=100.0).suffix("%")).changed() {
            o.background[3] = (alpha / 100.0 * 255.0).round() as u8;
        }
        ui.end_row();
        ui.label("Border width");
        ui.add(egui::Slider::new(&mut o.border_width, 0.0..=6.0));
        ui.end_row();
        ui.label("Corner rounding");
        ui.add(egui::Slider::new(&mut o.rounding, 0.0..=40.0));
        ui.end_row();
        ui.label("Padding");
        ui.add(egui::Slider::new(&mut o.padding, 0.0..=40.0));
        ui.end_row();
        ui.label("Spacing between stats");
        ui.add(egui::Slider::new(&mut o.spacing, 0.0..=60.0));
        ui.end_row();
    });
    ui.add_space(10.0);
    if ui.button("Reset overlay to defaults").clicked() {
        *o = OverlaySettings::default();
    }
}

fn session_settings(ui: &mut egui::Ui, t: &Theme, s: &mut SessionSettings) {
    section(ui, t, "Session detection");
    egui::Grid::new("sess").num_columns(3).spacing([16.0, 10.0]).show(ui, |ui| {
        ui.label("End session after no typing for");
        ui.add(egui::DragValue::new(&mut s.idle_timeout_secs).range(5.0..=3600.0).speed(1.0).suffix(" s"));
        ui.label(weak(t, "A new session starts with the next key press."));
        ui.end_row();
        ui.label("Minimum characters to save");
        ui.add(egui::DragValue::new(&mut s.min_chars_to_save).range(0..=10000));
        ui.label(weak(t, "Shorter sessions are discarded (e.g. a quick password)."));
        ui.end_row();
    });
    section(ui, t, "Measurement");
    egui::Grid::new("meas").num_columns(3).spacing([16.0, 10.0]).show(ui, |ui| {
        ui.label("Live WPM window");
        ui.add(egui::DragValue::new(&mut s.live_window_secs).range(2.0..=120.0).speed(0.5).suffix(" s"));
        ui.label(weak(t, "Live WPM averages over this many recent seconds."));
        ui.end_row();
        ui.label("Pause threshold");
        ui.add(egui::DragValue::new(&mut s.active_gap_secs).range(0.5..=60.0).speed(0.1).suffix(" s"));
        ui.label(weak(t, "Gaps longer than this don't count toward active typing time."));
        ui.end_row();
        ui.label("Graph sample interval");
        ui.add(egui::DragValue::new(&mut s.sample_secs).range(1.0..=60.0).speed(0.2).suffix(" s"));
        ui.label(weak(t, "How often the WPM graph records a point."));
        ui.end_row();
    });
    ui.add_space(8.0);
    ui.label(weak(
        t,
        "WPM uses the standard definition: 5 characters (including spaces) = 1 word. \
         The average uses active typing time only.",
    ));
    if ui.button("Reset to defaults").clicked() {
        *s = SessionSettings::default();
    }
}

fn hotkey_settings(ui: &mut egui::Ui, t: &Theme, settings: &mut Settings) {
    section(ui, t, "Global hotkeys");
    ui.label(weak(
        t,
        "These work from any app. Format: modifiers + key, e.g. Ctrl+Alt+P, Ctrl+Shift+F8, F9. \
         Matching key presses are not passed on to other apps. Leave a field empty to disable it.",
    ));
    ui.add_space(6.0);
    let parsed: Vec<(HotkeyAction, Option<Result<Hotkey, String>>)> = HotkeyAction::ALL
        .iter()
        .map(|&a| {
            let s = settings.hotkeys.get(a).trim();
            (a, (!s.is_empty()).then(|| Hotkey::parse(s)))
        })
        .collect();
    egui::Grid::new("hotkeys").num_columns(3).spacing([16.0, 10.0]).show(ui, |ui| {
        for (action, result) in parsed.iter() {
            ui.label(action.label());
            ui.add(egui::TextEdit::singleline(settings.hotkeys.get_mut(*action)).desired_width(180.0));
            match result {
                None => ui.label(weak(t, "Disabled")),
                Some(Ok(hk)) => {
                    let dup = parsed.iter().any(|(a, r)| {
                        a != action && matches!(r, Some(Ok(other)) if other == hk)
                    });
                    if dup {
                        ui.colored_label(Color32::from_rgb(234, 179, 8), "Used twice")
                    } else {
                        ui.colored_label(Color32::from_rgb(74, 222, 128), format!("OK: {}", hk.display()))
                    }
                }
                Some(Err(e)) => ui.colored_label(Color32::from_rgb(239, 68, 68), e),
            };
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    if ui.button("Reset to defaults").clicked() {
        settings.hotkeys = Default::default();
    }
}
