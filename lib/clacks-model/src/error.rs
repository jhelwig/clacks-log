#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    #[error("invalid recognition catalog: {reason}")]
    InvalidRecognitionCatalog { reason: String },
    #[error("{axis} outside geographic range: {value}")]
    InvalidCoordinate { axis: String, value: String },
    #[error("invalid Maidenhead locator: {value}")]
    InvalidLocator { value: String },
    #[error("invalid locator length: {length}")]
    InvalidGridLength { length: u8 },
    #[error("invalid decimal degrees: {value}")]
    MalformedDecimal { value: String },
    #[error("invalid {system} zone: {value}")]
    InvalidZone { system: String, value: u8 },
    #[error("{field} cannot be blank")]
    EmptyText { field: String },
    #[error("{unit} must be positive")]
    ZeroQuantity { unit: String },
    #[error("invalid ID: {value}")]
    InvalidId { value: String },
    #[error("nanoseconds outside 0..1,000,000,000: {nanoseconds}")]
    InvalidTimestamp { nanoseconds: u32 },
}
