use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueError {
    EmptyText { field: String },
    ZeroQuantity { unit: String },
    InvalidId { value: String },
    InvalidTimestamp { nanoseconds: u32 },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
