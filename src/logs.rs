use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const MAX_LOG_LINES: usize = 200;

pub type LogBuffer = Arc<Mutex<VecDeque<String>>>;

pub fn new_buffer() -> LogBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES)))
}

pub fn push(buf: &LogBuffer, line: String) {
    if let Ok(mut guard) = buf.lock() {
        if guard.len() >= MAX_LOG_LINES {
            guard.pop_front();
        }
        guard.push_back(line);
    }
}

pub fn snapshot(buf: &LogBuffer) -> Vec<String> {
    buf.lock().map(|g| g.iter().cloned().collect()).unwrap_or_default()
}

/// Tracing layer that captures INFO+ events from spytti and WARN+ from librespot
/// into the shared log buffer.
pub struct CaptureLayer {
    buf: LogBuffer,
}

impl CaptureLayer {
    pub fn new(buf: LogBuffer) -> Self {
        Self { buf }
    }
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let target = meta.target();
        let level = *meta.level();

        // Filter: spytti INFO+, librespot_discovery DEBUG+ (catch Zeroconf HTTP),
        // other librespot WARN+ (MAC mismatch, dealer errors, etc.)
        let keep = if target.starts_with("spytti") {
            level <= tracing::Level::INFO
        } else if target.starts_with("librespot_discovery") {
            level <= tracing::Level::DEBUG
        } else if target.starts_with("librespot") {
            level <= tracing::Level::WARN
        } else {
            false
        };
        if !keep {
            return;
        }

        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);

        let ts = chrono_now();
        let line = format!("{ts} {level:5} {target}: {}", visitor.0);
        push(&self.buf, line);
    }
}

struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
            // Strip surrounding quotes that Debug adds for strings
            if self.0.starts_with('"') && self.0.ends_with('"') && self.0.len() >= 2 {
                self.0 = self.0[1..self.0.len() - 1].to_string();
            }
        }
    }
}

/// Simple timestamp without pulling in chrono: seconds since start.
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_respects_max() {
        let buf = new_buffer();
        for i in 0..250 {
            push(&buf, format!("line {i}"));
        }
        let snap = snapshot(&buf);
        assert_eq!(snap.len(), MAX_LOG_LINES);
        assert_eq!(snap[0], "line 50");
        assert_eq!(snap[MAX_LOG_LINES - 1], "line 249");
    }

    #[test]
    fn snapshot_returns_all_pushed() {
        let buf = new_buffer();
        push(&buf, "first".into());
        push(&buf, "second".into());
        assert_eq!(snapshot(&buf), vec!["first", "second"]);
    }
}
