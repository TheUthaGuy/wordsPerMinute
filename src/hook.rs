//! System-wide keyboard hook (WH_KEYBOARD_LL).
//!
//! Privacy: only the *category* of each key (character, word separator,
//! backspace) and its timestamp leave this module. The actual key is never
//! stored or forwarded, so typed text cannot be reconstructed.

use crate::hotkey::{Hotkey, HotkeyAction};
use eframe::egui;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    /// A printable character (letter, digit, punctuation).
    Char,
    /// Space, Enter or Tab: ends a word.
    Separator,
    Backspace,
}

pub enum HookEvent {
    Key { kind: KeyKind, at: Instant },
    Hotkey(HotkeyAction),
}

struct Shared {
    tx: Sender<HookEvent>,
    ctx: egui::Context,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
static HOTKEYS: Mutex<Vec<(Hotkey, HotkeyAction)>> = Mutex::new(Vec::new());

pub fn set_hotkeys(list: Vec<(Hotkey, HotkeyAction)>) {
    if let Ok(mut h) = HOTKEYS.lock() {
        *h = list;
    }
}

pub fn start(tx: Sender<HookEvent>, ctx: egui::Context) {
    let _ = SHARED.set(Shared { tx, ctx });
    std::thread::Builder::new()
        .name("keyboard-hook".into())
        .spawn(|| unsafe {
            let hmod = GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hmod, 0);
            if hook.is_null() {
                eprintln!("Failed to install keyboard hook");
                return;
            }
            // Low-level hooks are called on this thread, so it needs a message loop.
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            UnhookWindowsHookEx(hook);
        })
        .expect("failed to spawn keyboard hook thread");
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let msg = wparam as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            // Ignore synthetic input (remote desktop tools, macros, etc.).
            if kb.flags & LLKHF_INJECTED == 0 && handle_key(kb.vkCode as u16) {
                return 1; // swallow: it was one of our hotkeys
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

fn key_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

fn is_modifier(vk: u16) -> bool {
    [
        VK_SHIFT, VK_CONTROL, VK_MENU, VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU,
        VK_RMENU, VK_LWIN, VK_RWIN, VK_CAPITAL,
    ]
    .contains(&vk)
}

fn classify(vk: u16) -> Option<KeyKind> {
    match vk {
        0x08 => Some(KeyKind::Backspace),
        0x09 | 0x0D | 0x20 => Some(KeyKind::Separator),
        0x30..=0x39 // digits
        | 0x41..=0x5A // letters
        | 0x60..=0x6F // numpad
        | 0xBA..=0xC0 // OEM punctuation
        | 0xDB..=0xDF
        | 0xE2 => Some(KeyKind::Char),
        _ => None,
    }
}

fn own_window_focused() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid == GetCurrentProcessId()
    }
}

/// Returns true if the key should be swallowed.
fn handle_key(vk: u16) -> bool {
    let Some(shared) = SHARED.get() else {
        return false;
    };
    let ctrl = key_down(VK_CONTROL);
    let alt = key_down(VK_MENU);
    let shift = key_down(VK_SHIFT);
    let win = key_down(VK_LWIN) || key_down(VK_RWIN);

    if !is_modifier(vk) {
        let pressed = Hotkey { ctrl, alt, shift, win, vk };
        let action = HOTKEYS
            .lock()
            .ok()
            .and_then(|h| h.iter().find(|(k, _)| *k == pressed).map(|(_, a)| *a));
        if let Some(action) = action {
            let _ = shared.tx.send(HookEvent::Hotkey(action));
            shared.ctx.request_repaint();
            return true;
        }
    }

    // Don't count typing inside the WPM app itself (e.g. editing settings).
    if own_window_focused() {
        return false;
    }

    // Skip shortcuts like Ctrl+C. Ctrl+Alt together is AltGr on many layouts,
    // which produces real characters, so that still counts.
    let altgr = ctrl && alt;
    if (ctrl || alt || win) && !altgr {
        return false;
    }

    if let Some(kind) = classify(vk) {
        let _ = shared.tx.send(HookEvent::Key { kind, at: Instant::now() });
        shared.ctx.request_repaint();
    }
    false
}
