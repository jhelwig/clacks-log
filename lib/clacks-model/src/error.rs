//! Structured diagnostics for invalid native scalar values.

/// A scalar or recognition-catalog value that cannot satisfy its native invariant.
///
/// Use the variant and supplied input to present a field-specific diagnostic.
/// These failures do not imply a transport, storage or authentication error.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    /// A supplied recognition catalog is malformed or internally inconsistent.
    #[error("invalid recognition catalog: {reason}")]
    InvalidRecognitionCatalog {
        /// Catalog validation or decoding diagnostic.
        reason: String,
    },
    /// A geographic axis is outside its permitted range.
    #[error("{axis} outside geographic range: {value}")]
    InvalidCoordinate {
        /// Axis whose range was exceeded.
        axis: String,
        /// Supplied coordinate text for diagnostics.
        value: String,
    },
    /// A locator has an unsupported length or invalid character pair.
    #[error("invalid Maidenhead locator: {value}")]
    InvalidLocator {
        /// Rejected locator text after ASCII case normalization.
        value: String,
    },
    /// A locator length does not meet the applicable precision rule.
    #[error("invalid locator length: {length}")]
    InvalidGridLength {
        /// Rejected character count.
        length: u8,
    },
    /// Text is not a supported decimal-degree value.
    #[error("invalid decimal degrees: {value}")]
    MalformedDecimal {
        /// Original rejected decimal text.
        value: String,
    },
    /// A zone number is outside its zone system’s range.
    #[error("invalid {system} zone: {value}")]
    InvalidZone {
        /// Zone system identifying the permitted range.
        system: String,
        /// Rejected zone number.
        value: u8,
    },
    /// A required native text value is blank.
    #[error("{field} cannot be blank")]
    EmptyText {
        /// Field whose required text is absent.
        field: String,
    },
    /// A strictly positive physical quantity was zero.
    #[error("{unit} must be positive")]
    ZeroQuantity {
        /// Measurement unit used in the diagnostic.
        unit: String,
    },
    /// A physical quantity exceeds the signed 64-bit storage and domain limit.
    #[error("{unit} exceeds 9,223,372,036,854,775,807: {value}")]
    QuantityOutOfRange {
        /// Measurement unit identifying the invalid value.
        unit: String,
        /// Rejected whole-unit measurement.
        value: u64,
    },
    /// Text cannot be parsed as a native entity identity.
    #[error("invalid ID: {value}")]
    InvalidId {
        /// Rejected ID text.
        value: String,
    },
    /// A timestamp fraction is not within one second.
    #[error("nanoseconds outside 0..1,000,000,000: {nanoseconds}")]
    InvalidTimestamp {
        /// Rejected fractional nanosecond count.
        nanoseconds: u32,
    },
}
