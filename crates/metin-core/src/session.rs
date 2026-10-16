use serde::{Deserialize, Serialize};
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Statistics {
    pub active_seconds: f64,
    pub completed: u64,
    pub clicks: u64,
    pub failed: u64,
}
#[derive(Debug, Default, Clone)]
pub struct Session {
    pub active_seconds: f64,
    pub completed: u64,
    pub clicks: u64,
    pub failed: u64,
    last_time: Option<f64>,
    was_running: bool,
}
impl Session {
    pub fn advance(&mut self, now: f64, running: bool) {
        if self.was_running
            && let Some(last) = self.last_time
        {
            self.active_seconds += (now - last).max(0.);
        }
        self.last_time = Some(now);
        self.was_running = running;
    }
    pub fn pause(&mut self, now: Option<f64>) {
        if let Some(now) = now {
            self.advance(now, false);
        }
        self.was_running = false;
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn snapshot(&self) -> Statistics {
        Statistics {
            active_seconds: self.active_seconds,
            completed: self.completed,
            clicks: self.clicks,
            failed: self.failed,
        }
    }
}
pub fn duration(seconds: f64) -> String {
    let s = seconds.max(0.) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
