//! The `SOLID_TYPEFACTS_TIMINGS` stderr trace. The seam mirrors the Go
//! producer's `typefacts.Trace`; the stderr adapter is the only place that
//! reads the environment or writes to stderr.

use crate::gocompat::quote;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

pub trait Trace: Send + Sync {
    fn stage(&self, name: &str, elapsed: Duration);
    fn metrics(&self, name: &str, values: &[(&str, i64)]);
}

pub type SharedTrace = Option<Arc<dyn Trace>>;

pub struct StderrTrace;

/// Resolves the tracing decision once: `None` unless the variable is set and
/// non-empty.
#[must_use]
pub fn from_environment() -> SharedTrace {
    match std::env::var_os("SOLID_TYPEFACTS_TIMINGS") {
        Some(value) if !value.is_empty() => Some(Arc::new(StderrTrace)),
        _ => None,
    }
}

#[must_use]
pub fn stage_line(name: &str, elapsed: Duration) -> String {
    format!(
        "{{\"typefactsStage\":{},\"elapsedNs\":{}}}\n",
        quote(name),
        elapsed.as_nanos()
    )
}

#[must_use]
pub fn metrics_line(name: &str, values: &[(&str, i64)]) -> String {
    let mut line = format!("{{{}:{{", quote(&format!("typefacts:{name}")));
    for (index, (key, value)) in values.iter().enumerate() {
        if index > 0 {
            line.push(',');
        }
        line.push_str(&format!("{}:{value}", quote(key)));
    }
    line.push_str("}}\n");
    line
}

/// Consecutive phases of one operation, reported as one metrics line of
/// `<name>Ns` fields when a trace is listening (and timed only then).
pub struct Phases {
    last: Option<std::time::Instant>,
    phases: Vec<(&'static str, i64)>,
}

impl Phases {
    #[must_use]
    pub fn new(trace: &SharedTrace) -> Self {
        Self {
            last: trace.is_some().then(std::time::Instant::now),
            phases: Vec::new(),
        }
    }

    /// Ends the phase `name` (a field name ending in `Ns`).
    pub fn mark(&mut self, name: &'static str) {
        if let Some(last) = &mut self.last {
            let now = std::time::Instant::now();
            let elapsed = i64::try_from((now - *last).as_nanos()).unwrap_or(i64::MAX);
            self.phases.push((name, elapsed));
            *last = now;
        }
    }

    pub fn report(&self, trace: &SharedTrace, metrics: &str) {
        if let Some(trace) = trace {
            trace.metrics(metrics, &self.phases);
        }
    }
}

impl Trace for StderrTrace {
    fn stage(&self, name: &str, elapsed: Duration) {
        let _ = std::io::stderr().write_all(stage_line(name, elapsed).as_bytes());
    }

    fn metrics(&self, name: &str, values: &[(&str, i64)]) {
        let _ = std::io::stderr().write_all(metrics_line(name, values).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_match_the_go_format() {
        assert_eq!(
            stage_line("handshake-written", Duration::from_nanos(42)),
            "{\"typefactsStage\":\"handshake-written\",\"elapsedNs\":42}\n"
        );
        assert_eq!(
            metrics_line("closure", &[("files", 3), ("rows", -1)]),
            "{\"typefacts:closure\":{\"files\":3,\"rows\":-1}}\n"
        );
        assert_eq!(metrics_line("empty", &[]), "{\"typefacts:empty\":{}}\n");
    }
}
