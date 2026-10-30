//! Checked construction and read access for frozen LoTW submissions.

use super::*;

impl TryFrom<LotwSubmissionFields> for LotwSubmission {
    type Error = anyhow::Error;

    fn try_from(fields: LotwSubmissionFields) -> anyhow::Result<Self> {
        let invalid = |field: &str| ServiceError::Invalid {
            detail: format!("LoTW submission {field} differs from its frozen identity"),
        };
        anyhow::ensure!(
            fields.key.own_call == fields.signing_identity.callsign,
            invalid("own callsign")
        );
        let (date, time) = adif::contact_time(fields.key.start)?;
        for (name, expected) in [
            ("CALL", fields.key.worked_call.as_str()),
            ("BAND", fields.key.band.as_str()),
            ("MODE", fields.key.mode.as_str()),
            ("QSO_DATE", date.as_str()),
            ("QSO_TIME", time.as_str()),
        ] {
            anyhow::ensure!(
                fields.contact_fields.get(name).map(|v| v.trim_ascii()) == Some(expected),
                invalid(name)
            );
        }
        let receive = fields
            .contact_fields
            .get("BAND_RX")
            .map(|v| v.trim_ascii())
            .unwrap_or(&fields.key.band);
        anyhow::ensure!(receive == fields.key.receive_band, invalid("BAND_RX"));
        Ok(Self {
            mapping_provenance: fields.mapping_provenance,
            destination: fields.destination,
            signing_identity: fields.signing_identity,
            contact_fields: fields.contact_fields,
            station_fields: fields.station_fields,
            key: fields.key,
        })
    }
}

impl LotwSubmission {
    /// Return the definitions used to freeze these values.
    pub fn mapping_provenance(&self) -> &LotwMappingProvenance {
        &self.mapping_provenance
    }

    /// Return the remote collection selected for this submission.
    pub fn destination(&self) -> crate::ids::DestinationId {
        self.destination
    }

    /// Return the actual certificate identity selected for signing.
    pub fn signing_identity(&self) -> &PublicSigningIdentity {
        &self.signing_identity
    }

    /// Return the frozen contact fields, without protocol framing.
    pub fn contact_fields(&self) -> &AdifFields {
        &self.contact_fields
    }

    /// Return the frozen station fields, without protocol framing.
    pub fn station_fields(&self) -> &AdifFields {
        &self.station_fields
    }

    /// Return the exact remote identity frozen with these values.
    pub fn key(&self) -> &LotwRecordKey {
        &self.key
    }

    /// Recover editable input for a complete checked replacement.
    pub fn into_fields(self) -> LotwSubmissionFields {
        LotwSubmissionFields {
            mapping_provenance: self.mapping_provenance,
            destination: self.destination,
            signing_identity: self.signing_identity,
            contact_fields: self.contact_fields,
            station_fields: self.station_fields,
            key: self.key,
        }
    }

    /// Identify the remote record without reinterpreting historical mappings.
    pub fn external_key(&self) -> ExternalRecordKey {
        ExternalRecordKey {
            destination: self.destination,
            record: RemoteRecordId::Lotw(self.key.clone()),
        }
    }
}
