use crate::hook::KeyKind;
use crate::settings::SessionSettings;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A finished, saved session. Contains counts only, never typed text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub active_secs: f64,
    /// Printable characters (letters, digits, punctuation).
    pub chars: u64,
    /// Spaces, enters and tabs.
    pub separators: u64,
    pub words: u64,
    pub keystrokes: u64,
    pub backspaces: u64,
    pub peak_wpm: f64,
    /// `[seconds since start, rolling WPM]` samples.
    #[serde(default)]
    pub timeline: Vec<[f32; 2]>,
}

/// Standard WPM: one "word" = 5 characters (spaces included).
fn wpm(typed: u64, secs: f64) -> f64 {
    if secs < 2.0 {
        0.0
    } else {
        typed as f64 / 5.0 / (secs / 60.0)
    }
}

impl SessionRecord {
    pub fn typed(&self) -> u64 {
        self.chars + self.separators
    }
    pub fn avg_wpm(&self) -> f64 {
        wpm(self.typed(), self.active_secs)
    }
    pub fn duration_secs(&self) -> f64 {
        (self.end - self.start).num_milliseconds() as f64 / 1000.0
    }
    pub fn correction_rate(&self) -> f64 {
        if self.keystrokes == 0 {
            0.0
        } else {
            self.backspaces as f64 / self.keystrokes as f64 * 100.0
        }
    }
    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            format!("Session {}", self.start.format("%b %d, %H:%M"))
        } else {
            self.name.clone()
        }
    }
}

pub struct LiveSession {
    pub start_wall: DateTime<Local>,
    pub start: Instant,
    pub last_key: Instant,
    pub active_secs: f64,
    pub chars: u64,
    pub separators: u64,
    words: u64,
    in_word: bool,
    pub keystrokes: u64,
    pub backspaces: u64,
    pub peak_wpm: f64,
    /// Timestamps of recent characters, for the rolling WPM.
    recent: VecDeque<Instant>,
    pub timeline: Vec<[f32; 2]>,
    next_sample: f64,
}

impl LiveSession {
    fn new(at: Instant) -> Self {
        let ago = Instant::now().saturating_duration_since(at);
        let start_wall = Local::now() - chrono::Duration::from_std(ago).unwrap_or_default();
        Self {
            start_wall,
            start: at,
            last_key: at,
            active_secs: 0.0,
            chars: 0,
            separators: 0,
            words: 0,
            in_word: false,
            keystrokes: 0,
            backspaces: 0,
            peak_wpm: 0.0,
            recent: VecDeque::new(),
            timeline: Vec::new(),
            next_sample: 0.0,
        }
    }

    fn register(&mut self, kind: KeyKind, at: Instant, cfg: &SessionSettings) {
        if self.keystrokes > 0 {
            let gap = at.saturating_duration_since(self.last_key).as_secs_f64();
            if gap <= cfg.active_gap_secs as f64 {
                self.active_secs += gap;
            }
        }
        self.last_key = self.last_key.max(at);
        self.keystrokes += 1;
        match kind {
            KeyKind::Char => {
                self.chars += 1;
                self.in_word = true;
                self.recent.push_back(at);
            }
            KeyKind::Separator => {
                self.separators += 1;
                if self.in_word {
                    self.words += 1;
                    self.in_word = false;
                }
                self.recent.push_back(at);
            }
            KeyKind::Backspace => self.backspaces += 1,
        }
    }

    pub fn word_count(&self) -> u64 {
        self.words + self.in_word as u64
    }

    pub fn typed(&self) -> u64 {
        self.chars + self.separators
    }

    pub fn avg_wpm(&self) -> f64 {
        wpm(self.typed(), self.active_secs)
    }

    pub fn live_wpm(&self, now: Instant, window_secs: f64) -> f64 {
        let cutoff = now.checked_sub(Duration::from_secs_f64(window_secs));
        let n = self
            .recent
            .iter()
            .rev()
            .take_while(|t| cutoff.map_or(true, |c| **t >= c))
            .count();
        // Early in a session the window isn't full yet, so use the elapsed time.
        let span = now
            .saturating_duration_since(self.start)
            .as_secs_f64()
            .min(window_secs)
            .max(2.0);
        n as f64 / 5.0 / (span / 60.0)
    }

    fn to_record(&self, window_secs: f64) -> SessionRecord {
        let duration = self.last_key.saturating_duration_since(self.start);
        let end = self.start_wall + chrono::Duration::from_std(duration).unwrap_or_default();
        let limit = (duration.as_secs_f64() + window_secs) as f32;
        let mut timeline: Vec<[f32; 2]> =
            self.timeline.iter().copied().filter(|p| p[0] <= limit).collect();
        while timeline.last().is_some_and(|p| p[1] < 0.5) {
            timeline.pop();
        }
        let avg = self.avg_wpm();
        SessionRecord {
            id: self.start_wall.timestamp_millis(),
            name: String::new(),
            start: self.start_wall,
            end,
            active_secs: self.active_secs,
            chars: self.chars,
            separators: self.separators,
            words: self.word_count(),
            keystrokes: self.keystrokes,
            backspaces: self.backspaces,
            peak_wpm: self.peak_wpm.max(avg),
            timeline,
        }
    }
}

#[derive(Default)]
pub struct Tracker {
    pub current: Option<LiveSession>,
    pub paused: bool,
    pub last_activity: Option<Instant>,
}

impl Tracker {
    /// Handles one key. Returns a finished session if this key started a new one.
    pub fn on_key(
        &mut self,
        kind: KeyKind,
        at: Instant,
        cfg: &SessionSettings,
    ) -> Option<SessionRecord> {
        if self.paused {
            return None;
        }
        self.last_activity = Some(at);
        let mut finished = None;
        if let Some(cur) = &self.current {
            if at.saturating_duration_since(cur.last_key).as_secs_f64()
                > cfg.idle_timeout_secs as f64
            {
                finished = self.finish(cfg);
            }
        }
        self.current
            .get_or_insert_with(|| LiveSession::new(at))
            .register(kind, at, cfg);
        finished
    }

    /// Periodic update: idle timeout, peak tracking and graph sampling.
    pub fn tick(&mut self, now: Instant, cfg: &SessionSettings) -> Option<SessionRecord> {
        let cur = self.current.as_mut()?;
        if now.saturating_duration_since(cur.last_key).as_secs_f64() > cfg.idle_timeout_secs as f64
        {
            return self.finish(cfg);
        }
        let window = cfg.live_window_secs as f64;
        let cutoff = now.checked_sub(Duration::from_secs_f64(window));
        while cur
            .recent
            .front()
            .is_some_and(|t| cutoff.is_some_and(|c| *t < c))
        {
            cur.recent.pop_front();
        }
        let elapsed = now.saturating_duration_since(cur.start).as_secs_f64();
        let live = cur.live_wpm(now, window);
        if elapsed >= window {
            cur.peak_wpm = cur.peak_wpm.max(live);
        }
        if elapsed >= cur.next_sample {
            cur.timeline.push([elapsed as f32, live as f32]);
            cur.next_sample = elapsed + cfg.sample_secs.max(1.0) as f64;
        }
        None
    }

    /// Ends the current session. Returns it if it is long enough to keep.
    pub fn finish(&mut self, cfg: &SessionSettings) -> Option<SessionRecord> {
        let cur = self.current.take()?;
        let rec = cur.to_record(cfg.live_window_secs as f64);
        (rec.typed() >= cfg.min_chars_to_save as u64).then_some(rec)
    }

    /// Pausing ends the current session so paused time never counts.
    pub fn set_paused(&mut self, paused: bool, cfg: &SessionSettings) -> Option<SessionRecord> {
        self.paused = paused;
        if paused {
            self.finish(cfg)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_and_wpm() {
        let cfg = SessionSettings { min_chars_to_save: 1, ..Default::default() };
        let mut t = Tracker::default();
        let t0 = Instant::now();
        // "hello world " typed at 100ms per key: 12 keys over 1.1s.
        let keys = "hello world ";
        for (i, ch) in keys.chars().enumerate() {
            let kind = if ch == ' ' { KeyKind::Separator } else { KeyKind::Char };
            t.on_key(kind, t0 + Duration::from_millis(100 * i as u64), &cfg);
        }
        t.on_key(KeyKind::Backspace, t0 + Duration::from_millis(1200), &cfg);
        let cur = t.current.as_ref().unwrap();
        assert_eq!(cur.word_count(), 2);
        assert_eq!(cur.typed(), 12);
        assert_eq!(cur.backspaces, 1);
        let rec = t.finish(&cfg).unwrap();
        assert_eq!(rec.words, 2);
        assert_eq!(rec.keystrokes, 13);
        assert!((rec.active_secs - 1.2).abs() < 1e-6);
    }

    #[test]
    fn idle_gap_starts_new_session() {
        let cfg = SessionSettings { min_chars_to_save: 1, idle_timeout_secs: 5.0, ..Default::default() };
        let mut t = Tracker::default();
        let t0 = Instant::now();
        assert!(t.on_key(KeyKind::Char, t0, &cfg).is_none());
        let done = t.on_key(KeyKind::Char, t0 + Duration::from_secs(10), &cfg);
        assert!(done.is_some());
        assert_eq!(t.current.as_ref().unwrap().chars, 1);
    }

    #[test]
    fn short_sessions_are_discarded() {
        let cfg = SessionSettings::default();
        let mut t = Tracker::default();
        t.on_key(KeyKind::Char, Instant::now(), &cfg);
        assert!(t.finish(&cfg).is_none());
    }
}
