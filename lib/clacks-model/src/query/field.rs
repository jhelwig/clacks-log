//! Operator-facing query vocabulary and matching defaults.

/// Matching rules for a field's values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    /// Literal contacted/own callsign prefix by default.
    Call,
    /// Literal substring by default.
    Text,
    /// Whole text label by default.
    Label,
    /// Whole-second timestamp or complete UTC calendar day.
    Time,
    /// Frequency threshold with megahertz as the default unit.
    Frequency,
    /// Power threshold with watts as the default unit.
    Power,
    /// Catalog-interpreted mode family or specialization.
    Mode,
    /// Geographic known-within predicate.
    Grid,
    /// Group-qualified reference identifier.
    Reference,
    /// Current derived service-positive flag.
    Boolean,
}

/// One supported field's public spelling and operator help.
#[derive(Clone, Copy, Debug)]
pub struct FieldDefinition {
    /// Closed identity used by execution adapters.
    pub field: Field,
    /// Case-insensitive token name.
    pub name: &'static str,
    /// Operator-facing meaning, including source binding.
    pub description: &'static str,
    /// Accepted value and default matching kind.
    pub kind: FieldType,
}

/// Keep the closed field identities and their operator metadata in one table.
macro_rules! fields {
    ($( $variant:ident, $name:literal, $kind:ident, $description:literal; )*) => {
        /// Supported operator fields; no user text becomes a database identifier.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Field {
            $(#[doc = $description] $variant,)*
        }

        impl Field {
            /// Return this field's accepted value and default matching kind.
            pub fn kind(self) -> FieldType {
                match self { $(Self::$variant => FieldType::$kind,)* }
            }
        }

        /// Browse the vocabulary used for validation, help and suggestions.
        pub fn definitions() -> &'static [FieldDefinition] {
            &[$(FieldDefinition { field: Field::$variant, name: $name,
                description: $description, kind: FieldType::$kind },)*]
        }
    };
}

fields! {
    OwnCall, "own.call", Call, "Recorded own station callsign";
    ContactCall, "contact.call", Call, "Recorded contacted station callsign";
    Start, "qso_start", Time, "Saved contact start, UTC";
    End, "qso_end", Time, "Saved contact end, when known";
    Name, "name", Text, "Effective contacted operator name";
    RecordedName, "recorded.name", Text, "Recorded contacted operator name without fallback";
    Comment, "comment", Text, "Recorded contact comment";
    Tag, "tag", Label, "Membership by exact logbook name, including duplicate names";
    Frequency, "frequency", Frequency, "Primary frequency, MHz unless a unit is supplied";
    ReceiveFrequency, "receive.frequency", Frequency, "Receive override frequency, otherwise primary";
    RecordedReceiveFrequency, "recorded.receive.frequency", Frequency, "Recorded receive override frequency only";
    Band, "band", Label, "Primary band label";
    ReceiveBand, "receive.band", Label, "Receive override band, otherwise primary";
    RecordedReceiveBand, "recorded.receive.band", Label, "Recorded receive override band only";
    Power, "power", Power, "Recorded transmit power, watts unless a unit is supplied";
    Mode, "mode", Mode, "Operating mode family or specialization";
    OwnGrid, "own.grid", Grid, "Effective own location known to be within a grid";
    ContactGrid, "contact.grid", Grid, "Effective contacted location known to be within a grid";
    RecordedOwnGrid, "recorded.own.grid", Grid, "Recorded own location known to be within a grid";
    RecordedContactGrid, "recorded.contact.grid", Grid, "Recorded contacted location known to be within a grid";
    OwnActivity, "own.activity", Label, "Own recorded participation by group name";
    ContactActivity, "contact.activity", Label, "Contacted recorded participation by group name";
    OwnReference, "own.reference", Reference, "Own recorded group/reference identifier";
    ContactReference, "contact.reference", Reference, "Contacted recorded group/reference identifier";
    LotwUploaded, "lotw.uploaded", Boolean, "Current positive LoTW upload coverage";
    LotwConfirmed, "lotw.confirmed", Boolean, "Current applicable positive LoTW confirmation";
    QrzUploaded, "qrz.uploaded", Boolean, "Current positive coverage in the applicable QRZ book";
    QrzConfirmed, "qrz.confirmed", Boolean, "Current applicable QRZ confirmation";
    Sent, "sent_report", Text, "Recorded sent signal report";
    Received, "received_report", Text, "Recorded received signal report";
    Equipment, "equipment", Text, "Copied own equipment description";
    Antenna, "antenna", Text, "Copied own antenna description";
    EquipmentNotes, "equipment.notes", Text, "Copied own equipment notes";
    OwnOperatorCall, "own.operator.call", Call, "Known recorded own operator callsign";
    ContactOperatorCall, "contact.operator.call", Call, "Known recorded contacted operator callsign";
    OwnOperatorName, "own.operator.name", Text, "Recorded own operator name";
    ContactOperatorName, "contact.operator.name", Text, "Recorded contacted operator name";
}

impl Field {
    /// Resolve a token name without accepting arbitrary native property paths.
    pub fn named(name: &str) -> Option<Self> {
        definitions()
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .map(|d| d.field)
    }
}
