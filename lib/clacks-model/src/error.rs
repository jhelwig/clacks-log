use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueError {
    InvalidCoordinate { axis: String, value: String },
    InvalidLocator { value: String },
    InvalidGridLength { length: u8 },
    MalformedDecimal { value: String },
    InvalidZone { system: String, value: u8 },
    EmptyText { field: String },
    ZeroQuantity { unit: String },
    InvalidId { value: String },
    InvalidTimestamp { nanoseconds: u32 },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCoordinate { axis, value } => {
                write!(f, "{axis} outside geographic range: {value}")
            }
            Self::InvalidLocator { value } => write!(f, "invalid Maidenhead locator: {value}"),
            Self::InvalidGridLength { length } => write!(f, "invalid locator length: {length}"),
            Self::MalformedDecimal { value } => write!(f, "invalid decimal degrees: {value}"),
            Self::InvalidZone { system, value } => write!(f, "invalid {system} zone: {value}"),
            Self::EmptyText { field } => write!(f, "{field} cannot be blank"),
            Self::ZeroQuantity { unit } => write!(f, "{unit} must be positive"),
            Self::InvalidId { value } => write!(f, "invalid ID: {value}"),
            Self::InvalidTimestamp { nanoseconds } => {
                write!(f, "nanoseconds outside 0..1,000,000,000: {nanoseconds}")
            }
        }
    }
}

impl std::error::Error for ValueError {}
