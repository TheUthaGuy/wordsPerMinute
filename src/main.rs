// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dashboard;
mod hook;
mod hotkey;
mod icon;
mod settings;
mod stats;
mod storage;
mod theme;
mod tracker;
mod tray;

use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    if !acquire_single_instance() {
        // Another copy is already running; two hooks would double count.
        return Ok(());
    }

    let settings = storage::load_settings();
    let sessions = storage::load_sessions();
    let (tx, rx) = std::sync::mpsc::channel();
    let icon = Arc::new(icon::app_icon(64));

    // The root viewport is the floating overlay. The dashboard is a secondary
    // viewport that can be opened and closed without stopping the app.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app::OVERLAY_TITLE)
            .with_inner_size([220.0, 70.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_mouse_passthrough(true)
            .with_icon(icon.clone()),
        ..Default::default()
    };

    eframe::run_native(
        "WPM Counter",
        options,
        Box::new(move |cc| {
            hook::start(tx, cc.egui_ctx.clone());
            Ok(Box::new(app::App::new(cc, settings, sessions, rx, icon)))
        }),
    )
}

/// Returns false if another instance already holds the named mutex.
fn acquire_single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let name: Vec<u16> = "Local\\WpmCounterSingleInstance\0".encode_utf16().collect();
    // The handle is intentionally leaked: it must live as long as the process.
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        !handle.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}
