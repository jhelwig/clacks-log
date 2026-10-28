//! Caret-local completion context without rewriting the whole expression.

use super::field::Field;
use std::ops::Range;

/// Meaning of the current completion range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionContext {
    /// A field name or unqualified callsign may be entered.
    Field,
    /// A typed value or supported comparison prefix is being edited.
    Value(Field),
    /// A presence predicate's field name is being edited.
    Presence,
}

/// Original replacement range and literal prefix for optional suggestions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionSite {
    /// Byte range replaced when accepting one suggestion.
    pub replacement: Range<usize>,
    /// Context used to choose the permitted suggestions.
    pub context: CompletionContext,
    /// Original text up to the caret within that replacement range.
    pub prefix: String,
}

/// Locate the active token even when quoting or the expression is unfinished.
///
/// A caret outside the text is clamped to its preceding UTF-8 boundary; callers
/// convert framework character cursors to byte offsets before requesting this.
pub fn site(text: &str, caret_byte: usize) -> CompletionSite {
    let mut caret = caret_byte.min(text.len());
    while !text.is_char_boundary(caret) {
        caret = caret.saturating_sub(1);
    }
    let before = text.get(..caret).unwrap_or("");
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in before.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if !quoted && (c.is_whitespace() || matches!(c, '(' | ')')) {
            start = i + c.len_utf8();
        }
    }
    if before
        .get(start..)
        .is_some_and(|token| token.starts_with('-'))
    {
        start += 1;
    }
    let token = before.get(start..).unwrap_or("");
    let (context, prefix) = if let Some((name, value)) = token.split_once(':') {
        start += name.len() + 1;
        (
            if name.eq_ignore_ascii_case("has") {
                CompletionContext::Presence
            } else {
                Field::named(name).map_or(CompletionContext::Field, CompletionContext::Value)
            },
            value,
        )
    } else {
        (CompletionContext::Field, token)
    };
    let mut end = caret;
    for (i, c) in text.get(caret..).unwrap_or("").char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if !quoted && (c.is_whitespace() || matches!(c, '(' | ')')) {
            break;
        }
        end = caret + i + c.len_utf8();
    }
    CompletionSite {
        replacement: start..end,
        context,
        prefix: prefix.to_owned(),
    }
}
