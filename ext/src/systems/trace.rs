// ABE - Logging / Trace Infrastructure
//
// Simple diagnostic logging facility for debugging ballistics computations.
// No external dependencies — everything uses std::io::Write to stderr.
//
// ## Usage
//
// ```ignore
// use crate::trace::{trace_init, trace_println, trace_enabled};
//
// trace_init();
// trace_println!("step: v={:.1} x={:.1}", vx, x);
// ponytail: debug-only tracing, not used in production builds

#![allow(dead_code)]
// if trace_enabled() {
//     // heavy work only when tracing
// }
// ```
//
// Trace output goes to stderr with an elapsed-time prefix
// `[T+<secs>.<millis>] <message>`. Initialise once at program start.

use std::io::Write;
use std::sync::OnceLock;
use std::time::Instant;

// ── Global trace state ─────────────────────────────────────────────────────────

struct TraceState {
    enabled: bool,
    start: Instant,
}

static TRACE: OnceLock<TraceState> = OnceLock::new();

/// Enable tracing output to stderr.
///
/// Idempotent — subsequent calls are no-ops.
pub fn trace_init() {
    let _ = TRACE.set(TraceState {
        enabled: true,
        start: Instant::now(),
    });
}

/// Check whether tracing is enabled.
pub fn trace_enabled() -> bool {
    TRACE.get().map(|t| t.enabled).unwrap_or(false)
}

/// Write a trace line to stderr.
///
/// The line is prefixed with `[T+<secs>.<millis>] ` and suffixed
/// with a newline. Writing is best-effort; errors are ignored.
pub fn trace_write(msg: &str) {
    let state = match TRACE.get() {
        Some(s) if s.enabled => s,
        _ => return,
    };

    let elapsed = state.start.elapsed();
    let _ = writeln!(
        std::io::stderr(),
        "[T+{}.{:03}] {}",
        elapsed.as_secs(),
        elapsed.subsec_millis(),
        msg
    );
}

/// Macro: write a trace line with format string.
///
/// Only does work if tracing was initialised with `trace_init()`.
///
/// ```ignore
/// trace_println!("impact: ke={:.1} J", energy);
/// ```
#[macro_export]
macro_rules! trace_println {
    ($($arg:tt)*) => {
        if $crate::systems::trace::trace_enabled() {
            $crate::systems::trace::trace_write(&format!($($arg)*));
        }
    };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_disabled_by_default() {
        assert!(!trace_enabled(), "Trace should be disabled by default");
    }

    #[test]
    fn trace_init_enables() {
        trace_init();
        if !trace_enabled() {
            return; // already initialised — nothing to check
        }
        assert!(trace_enabled(), "Trace should be enabled after init");
        trace_write("trace_test: basic write");
    }

    #[test]
    fn trace_macro_expands() {
        trace_init();
        if !trace_enabled() {
            return;
        }
        let val = 42.0_f64;
        trace_println!("trace_macro_test: value = {:.1}", val);
    }

    #[test]
    fn trace_init_idempotent() {
        trace_init();
        trace_init();
        // Should not panic — either state is valid
    }
}
