use crate::settings::Settings;
use crate::tracker::SessionRecord;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";
const SESSIONS_FILE: &str = "sessions.json";

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "WpmCounter")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_json<T: DeserializeOwned>(name: &str) -> Result<Option<T>, String> {
    let path = data_dir().join(name);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| e.to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Writes to a temp file then renames, so a crash never leaves a half-written file.
fn write_json<T: Serialize>(name: &str, value: &T) -> io::Result<()> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!("{name}.tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(tmp, dir.join(name))
}

/// Keeps a copy of an unreadable file instead of overwriting it.
fn back_up_corrupt(name: &str) {
    let dir = data_dir();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let _ = fs::rename(dir.join(name), dir.join(format!("{name}.corrupt-{stamp}")));
}

pub fn load_settings() -> Settings {
    let mut s = match read_json::<Settings>(SETTINGS_FILE) {
        Ok(Some(s)) => s,
        Ok(None) => Settings::default(),
        Err(_) => {
            back_up_corrupt(SETTINGS_FILE);
            Settings::default()
        }
    };
    s.normalize();
    s
}

pub fn save_settings(s: &Settings) -> io::Result<()> {
    write_json(SETTINGS_FILE, s)
}

pub fn load_sessions() -> Vec<SessionRecord> {
    match read_json::<Vec<SessionRecord>>(SESSIONS_FILE) {
        Ok(Some(mut v)) => {
            v.sort_by_key(|s| s.start);
            v
        }
        Ok(None) => Vec::new(),
        Err(_) => {
            back_up_corrupt(SESSIONS_FILE);
            Vec::new()
        }
    }
}

pub fn save_sessions(sessions: &[SessionRecord]) -> io::Result<()> {
    write_json(SESSIONS_FILE, &sessions)
}

pub fn export_json(path: &Path, sessions: &[SessionRecord]) -> io::Result<()> {
    fs::write(path, serde_json::to_string_pretty(sessions)?)
}

pub fn export_csv(path: &Path, sessions: &[SessionRecord]) -> io::Result<()> {
    let mut out = String::from(
        "start,end,name,duration_secs,active_secs,avg_wpm,peak_wpm,words,characters,keystrokes,backspaces,correction_rate_pct\n",
    );
    for s in sessions {
        let name = s.name.replace('"', "\"\"");
        out.push_str(&format!(
            "{},{},\"{}\",{:.0},{:.1},{:.1},{:.1},{},{},{},{},{:.2}\n",
            s.start.to_rfc3339(),
            s.end.to_rfc3339(),
            name,
            s.duration_secs(),
            s.active_secs,
            s.avg_wpm(),
            s.peak_wpm,
            s.words,
            s.typed(),
            s.keystrokes,
            s.backspaces,
            s.correction_rate(),
        ));
    }
    fs::write(path, out)
}
