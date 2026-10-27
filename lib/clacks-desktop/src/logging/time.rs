//! Whole-second UTC form controls with independent live and locked states.

use clacks_model::scalar::UtcSecond;

/// Editable UTC date/time and whether it follows the system clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContactTimestamp {
    /// Combined UTC input; blank represents an unknown optional end.
    pub text: String,
    /// Follow current UTC until explicitly locked.
    pub live: bool,
}

impl ContactTimestamp {
    /// Start a clock-following contact timestamp.
    pub fn live(now: UtcSecond) -> Self {
        Self {
            text: format(now),
            live: true,
        }
    }

    /// Display a saved timestamp without changing it as the clock advances.
    pub fn recorded(value: Option<UtcSecond>) -> Self {
        Self {
            text: value.map(format).unwrap_or_default(),
            live: false,
        }
    }

    /// Update only a live control; locked edits, including blank ends, stay intact.
    pub fn tick(&mut self, now: UtcSecond) -> anyhow::Result<()> {
        if self.live {
            self.text = format_checked(now)?;
        }
        Ok(())
    }

    /// Freeze the displayed complete UTC value for manual editing.
    pub fn lock(&mut self) {
        self.live = false;
    }

    /// Resume live date/time, replacing any locked value or unknown end.
    pub fn unlock(&mut self, now: UtcSecond) -> anyhow::Result<()> {
        self.live = true;
        self.tick(now)
    }

    /// Parse a complete UTC value, leaving requiredness to the contact form.
    pub fn value(&self) -> anyhow::Result<Option<UtcSecond>> {
        if self.text.trim().is_empty() {
            return Ok(None);
        }
        let format = time::format_description::parse_borrowed::<2>(
            "[year]-[month]-[day] [hour]:[minute]:[second]",
        )?;
        let value = time::PrimitiveDateTime::parse(self.text.trim(), &format)?.assume_utc();
        Ok(Some(UtcSecond::from_unix_seconds(value.unix_timestamp())))
    }
}

/// Display a UTC timestamp; out-of-calendar values retain their literal epoch value.
pub(crate) fn format(value: UtcSecond) -> String {
    format_checked(value).unwrap_or_else(|_| value.unix_seconds().to_string())
}

/// Format a calendar-representable timestamp, reporting unsupported ranges.
fn format_checked(value: UtcSecond) -> anyhow::Result<String> {
    let format = time::format_description::parse_borrowed::<2>(
        "[year]-[month]-[day] [hour]:[minute]:[second]",
    )?;
    Ok(time::OffsetDateTime::from_unix_timestamp(value.unix_seconds())?.format(&format)?)
}

/// Capture the current wall clock at the native whole-second resolution.
pub(crate) fn now() -> UtcSecond {
    UtcSecond::from_unix_seconds(time::OffsetDateTime::now_utc().unix_timestamp())
}
