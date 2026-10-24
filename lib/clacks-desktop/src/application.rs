//! Desktop application composition and its owner-startup measurement definition.

/// Seconds elapsed for one owning-process workspace startup attempt.
pub(crate) const STARTUP_DURATION_METRIC: &str = "clacks_desktop_startup_duration_seconds";

/// Finite second boundaries spanning ordinary startup and slow migration work.
pub(crate) const STARTUP_DURATION_BOUNDS: &[f64] =
    &[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];
