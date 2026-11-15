use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatKind {
    LiveWpm,
    AverageWpm,
    PeakWpm,
    Words,
    Characters,
    Keystrokes,
    Backspaces,
    CorrectionRate,
    Duration,
    ActiveTime,
    TodayWords,
    TodayTime,
    AllTimeWords,
}

impl StatKind {
    pub const ALL: [StatKind; 13] = [
        StatKind::LiveWpm,
        StatKind::AverageWpm,
        StatKind::PeakWpm,
        StatKind::Words,
        StatKind::Characters,
        StatKind::Keystrokes,
        StatKind::Backspaces,
        StatKind::CorrectionRate,
        StatKind::Duration,
        StatKind::ActiveTime,
        StatKind::TodayWords,
        StatKind::TodayTime,
        StatKind::AllTimeWords,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StatKind::LiveWpm => "Live WPM",
            StatKind::AverageWpm => "Average WPM",
            StatKind::PeakWpm => "Peak WPM",
            StatKind::Words => "Words",
            StatKind::Characters => "Characters",
            StatKind::Keystrokes => "Keystrokes",
            StatKind::Backspaces => "Backspaces",
            StatKind::CorrectionRate => "Correction rate",
            StatKind::Duration => "Session length",
            StatKind::ActiveTime => "Active typing time",
            StatKind::TodayWords => "Words today",
            StatKind::TodayTime => "Typing time today",
            StatKind::AllTimeWords => "Words all time",
        }
    }

    /// Short uppercase label for the overlay.
    pub fn short(self) -> &'static str {
        match self {
            StatKind::LiveWpm => "WPM",
            StatKind::AverageWpm => "AVG",
            StatKind::PeakWpm => "PEAK",
            StatKind::Words => "WORDS",
            StatKind::Characters => "CHARS",
            StatKind::Keystrokes => "KEYS",
            StatKind::Backspaces => "BKSP",
            StatKind::CorrectionRate => "CORR",
            StatKind::Duration => "TIME",
            StatKind::ActiveTime => "ACTIVE",
            StatKind::TodayWords => "TODAY",
            StatKind::TodayTime => "TODAY T",
            StatKind::AllTimeWords => "TOTAL",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            StatKind::LiveWpm => "Speed over the last few seconds (rolling window)",
            StatKind::AverageWpm => "Session average, counting only active typing time",
            StatKind::PeakWpm => "Fastest rolling WPM reached this session",
            StatKind::Words => "Words typed this session",
            StatKind::Characters => "Characters typed this session (incl. spaces)",
            StatKind::Keystrokes => "All counted keys incl. backspace",
            StatKind::Backspaces => "Backspace presses this session",
            StatKind::CorrectionRate => "Backspaces as a share of keystrokes",
            StatKind::Duration => "Wall-clock time since the session started",
            StatKind::ActiveTime => "Time spent actually typing (pauses excluded)",
            StatKind::TodayWords => "All words typed today",
            StatKind::TodayTime => "Active typing time today",
            StatKind::AllTimeWords => "Every word ever counted",
        }
    }
}

/// Values for every stat at one moment in time.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub live_wpm: f64,
    pub avg_wpm: f64,
    pub peak_wpm: f64,
    pub words: u64,
    pub chars: u64,
    pub keystrokes: u64,
    pub backspaces: u64,
    pub duration_secs: f64,
    pub active_secs: f64,
    pub today_words: u64,
    pub today_secs: f64,
    pub all_time_words: u64,
}

impl Snapshot {
    pub fn correction_rate(&self) -> f64 {
        if self.keystrokes == 0 {
            0.0
        } else {
            self.backspaces as f64 / self.keystrokes as f64 * 100.0
        }
    }

    pub fn value(&self, kind: StatKind) -> String {
        match kind {
            StatKind::LiveWpm => format!("{:.0}", self.live_wpm),
            StatKind::AverageWpm => format!("{:.0}", self.avg_wpm),
            StatKind::PeakWpm => format!("{:.0}", self.peak_wpm),
            StatKind::Words => fmt_num(self.words),
            StatKind::Characters => fmt_num(self.chars),
            StatKind::Keystrokes => fmt_num(self.keystrokes),
            StatKind::Backspaces => fmt_num(self.backspaces),
            StatKind::CorrectionRate => format!("{:.1}%", self.correction_rate()),
            StatKind::Duration => fmt_duration(self.duration_secs),
            StatKind::ActiveTime => fmt_duration(self.active_secs),
            StatKind::TodayWords => fmt_num(self.today_words),
            StatKind::TodayTime => fmt_duration(self.today_secs),
            StatKind::AllTimeWords => fmt_num(self.all_time_words),
        }
    }
}

pub fn fmt_duration(secs: f64) -> String {
    let total = secs.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn fmt_num(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(fmt_num(0), "0");
        assert_eq!(fmt_num(1234567), "1,234,567");
        assert_eq!(fmt_duration(65.0), "1:05");
        assert_eq!(fmt_duration(3725.0), "1:02:05");
    }
}
