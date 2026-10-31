//! Native field identities for operator-owned synchronization comparisons.

pub(super) mod qso;
pub(super) mod record;
pub(super) mod station;

use clacks_model::reference::StationSide;

/// An editable native field, independent of its rendered label or SQL layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FieldId {
    /// One recorded contact field.
    Qso(QsoField),
    /// Saved station profile field.
    Profile(ProfileField),
    /// Reusable equipment field.
    Equipment(EquipmentField),
    /// Named logbook field.
    Tag(TagField),
}

/// Station snapshot members; position remains one coupled native value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StationField {
    /// Recorded station identity.
    Callsign,
    /// Grid-area or explicit-point evidence with retained precision.
    Position,
    /// Recorded DXCC identifier.
    Dxcc,
    /// Recorded first-level administrative subdivision.
    State,
    /// Recorded second-level subdivision.
    County,
    /// Recorded CQ zone.
    Cq,
    /// Recorded ITU zone.
    Itu,
    /// Complete station-side participation associations.
    References,
}

/// Contact fields exposed by the existing logging and historical editors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum QsoField {
    /// Contact start instant.
    Start,
    /// Optional end instant.
    End,
    /// One side's station fact.
    Station {
        /// Own or contacted station.
        side: StationSide,
        /// Native member of that snapshot.
        field: StationField,
    },
    /// Recorded primary frequency/band pair.
    Primary,
    /// Optional receive override.
    Receive,
    /// Recorded mode hierarchy.
    Mode,
    /// Our optional operator callsign.
    OwnOperatorCall,
    /// Our optional recorded operator name.
    OwnOperatorName,
    /// Optional contacted operator callsign.
    ContactOperatorCall,
    /// Optional recorded contacted operator name.
    ContactOperatorName,
    /// Recorded transmit power.
    Power,
    /// Sent signal report.
    Sent,
    /// Received signal report.
    Received,
    /// General recorded comment.
    Comment,
    /// Copied equipment description.
    Equipment,
    /// Copied antenna description.
    Antenna,
    /// Copied equipment notes.
    EquipmentNotes,
    /// Complete logbook membership set.
    Tags,
}

impl FieldId {
    /// Use the configured radio-data font for identifiers, measurements and radio facts.
    pub fn radio_data(self) -> bool {
        !matches!(
            self,
            Self::Qso(
                QsoField::OwnOperatorName
                    | QsoField::ContactOperatorName
                    | QsoField::Comment
                    | QsoField::Equipment
                    | QsoField::Antenna
                    | QsoField::EquipmentNotes
            ) | Self::Profile(ProfileField::Name | ProfileField::DefaultEquipment)
                | Self::Equipment(_)
                | Self::Tag(_)
        )
    }

    /// Return the existing recorded-contact fields in display order.
    pub fn qso_fields() -> Vec<Self> {
        let mut fields = vec![
            QsoField::Start,
            QsoField::End,
            QsoField::Primary,
            QsoField::Receive,
            QsoField::Mode,
        ];
        for side in [StationSide::Contacted, StationSide::Own] {
            for field in [
                StationField::Callsign,
                StationField::Position,
                StationField::Dxcc,
                StationField::State,
                StationField::County,
                StationField::Cq,
                StationField::Itu,
                StationField::References,
            ] {
                fields.push(QsoField::Station { side, field });
            }
        }
        fields.extend([
            QsoField::OwnOperatorCall,
            QsoField::OwnOperatorName,
            QsoField::ContactOperatorCall,
            QsoField::ContactOperatorName,
            QsoField::Power,
            QsoField::Sent,
            QsoField::Received,
            QsoField::Comment,
            QsoField::Equipment,
            QsoField::Antenna,
            QsoField::EquipmentNotes,
            QsoField::Tags,
        ]);
        fields.into_iter().map(Self::Qso).collect()
    }

    /// Label the native field without treating its label as its identity.
    pub fn label(self) -> String {
        let field = match self {
            Self::Qso(field) => field,
            Self::Profile(ProfileField::Station(field)) => {
                return Self::Qso(QsoField::Station {
                    side: StationSide::Own,
                    field,
                })
                .label();
            }
            Self::Profile(ProfileField::Name)
            | Self::Equipment(EquipmentField::Name)
            | Self::Tag(TagField::Name) => return "Name".into(),
            Self::Profile(ProfileField::DefaultEquipment) => return "Default equipment".into(),
            Self::Equipment(EquipmentField::Description) => return "Equipment".into(),
            Self::Equipment(EquipmentField::Antenna) => return "Antenna".into(),
            Self::Equipment(EquipmentField::Notes) => return "Equipment notes".into(),
            Self::Tag(TagField::Lotw) => return "LoTW automatic uploads".into(),
            Self::Tag(TagField::Qrz) => return "QRZ automatic uploads".into(),
        };

        match field {
            QsoField::Station { side, field } => format!(
                "{} {}",
                match side {
                    StationSide::Own => "Our",
                    StationSide::Contacted => "Contact",
                },
                match field {
                    StationField::Callsign => "station callsign",
                    StationField::Position => "position",
                    StationField::Dxcc => "DXCC entity",
                    StationField::State => "state / province",
                    StationField::County => "county / subdivision",
                    StationField::Cq => "CQ zone",
                    StationField::Itu => "ITU zone",
                    StationField::References => "participation / references",
                }
            ),
            QsoField::Start => "Start · UTC".into(),
            QsoField::End => "End · UTC".into(),
            QsoField::Primary => "Frequency / band".into(),
            QsoField::Receive => "Receive override".into(),
            QsoField::Mode => "Mode".into(),
            QsoField::OwnOperatorCall => "Our operator callsign".into(),
            QsoField::OwnOperatorName => "Our operator name".into(),
            QsoField::ContactOperatorCall => "Contact operator callsign".into(),
            QsoField::ContactOperatorName => "Contact name".into(),
            QsoField::Power => "Transmit power · W".into(),
            QsoField::Sent => "Sent report".into(),
            QsoField::Received => "Received report".into(),
            QsoField::Comment => "Comment".into(),
            QsoField::Equipment => "Equipment".into(),
            QsoField::Antenna => "Antenna".into(),
            QsoField::EquipmentNotes => "Equipment notes".into(),
            QsoField::Tags => "Logbook membership".into(),
        }
    }
}

/// Individually resolved reusable station-profile facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProfileField {
    /// Display name.
    Name,
    /// One recorded station member.
    Station(StationField),
    /// Optional reusable equipment identity.
    DefaultEquipment,
}

/// Individually resolved equipment configuration facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EquipmentField {
    /// Display name.
    Name,
    /// Copied rig description.
    Description,
    /// Copied antenna description.
    Antenna,
    /// Equipment notes.
    Notes,
}

/// Named-tag facts; service policies are independent choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TagField {
    /// Display name.
    Name,
    /// LoTW automatic-upload contribution.
    Lotw,
    /// QRZ automatic-upload contribution.
    Qrz,
}
